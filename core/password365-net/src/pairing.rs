//! Pairing a new device over the local network, step by step so each step
//! can be a screen.
//!
//! Existing device ([`PairingHost`]):
//! 1. `open` listens and builds the invite; the app shows `invite_code()` as a QR code;
//! 2. `accept` waits for the new device and returns the verification code to show;
//! 3. once the user confirms the codes match and picks vaults, `approve` sends the
//!    join bundle and waits for the new device to report its identity, which it pins.
//!
//! New device ([`Joining`]):
//! 1. `connect` with the scanned code returns the same verification code;
//! 2. `receive_bundle` waits for the other device's approval;
//! 3. the app calls `Store::join` with the master password, then `complete`.

use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use password365_model::Timestamp;
use password365_sync::pairing::{Handshake, Invite, InviteError, StaticKey};
use password365_vault::wire::PairMessage;
use password365_vault::{Peer, Session, VaultError};
use zeroize::Zeroizing;

use crate::secure::SecureStream;
use crate::{NetError, Result};

/// People type a master password and compare codes during pairing; give them time.
pub const PAIRING_TIMEOUT: Duration = Duration::from_secs(300);
/// How often a cancellable wait checks for a connection or a cancel.
const POLL: Duration = Duration::from_millis(50);

/// The existing device's side, before the new device connects.
pub struct PairingHost {
    listener: TcpListener,
    invite: Invite,
    key: StaticKey,
}

impl PairingHost {
    /// Listens on `ip` (any free port) and prepares a one-time invite.
    pub fn open(session: &Session, ip: IpAddr) -> Result<Self> {
        let listener = TcpListener::bind(SocketAddr::new(ip, 0))?;
        let addr = listener.local_addr()?;
        let key = session.static_key();
        let invite = Invite::new(session.device_id(), key.public(), Some(addr.to_string()));
        Ok(Self {
            listener,
            invite,
            key,
        })
    }

    /// The text to show as a QR code (it contains a one-time secret).
    #[must_use]
    pub fn invite_code(&self) -> String {
        self.invite.to_code()
    }

    /// Waits for the new device and runs the handshake. Show the returned
    /// pairing's code and ask the user to compare it with the other screen.
    pub fn accept(self) -> Result<HostPairing> {
        let (socket, from) = self.listener.accept()?;
        self.handshake(socket, from)
    }

    /// Like [`Self::accept`], but gives up when `cancel` is set or after
    /// [`PAIRING_TIMEOUT`], closing the invite (its secret dies with it).
    pub fn accept_cancellable(self, cancel: &AtomicBool) -> Result<HostPairing> {
        self.listener.set_nonblocking(true)?;
        let deadline = Instant::now() + PAIRING_TIMEOUT;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(NetError::Cancelled);
            }
            match self.listener.accept() {
                Ok((socket, from)) => {
                    socket.set_nonblocking(false)?;
                    return self.handshake(socket, from);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::from(std::io::ErrorKind::TimedOut).into());
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn handshake(self, socket: TcpStream, from: SocketAddr) -> Result<HostPairing> {
        prepare(&socket)?;
        let stream = SecureStream::establish(socket, Handshake::inviter(&self.key, &self.invite)?)?;
        Ok(HostPairing {
            stream,
            from,
            vaults: Vec::new(),
        })
    }
}

/// The existing device's side once the new device is connected.
pub struct HostPairing {
    stream: SecureStream<TcpStream>,
    from: SocketAddr,
    /// The vaults sent to the new device: the only ones it will sync.
    vaults: Vec<password365_model::VaultId>,
}

impl HostPairing {
    /// Six digits to compare with the new device's screen.
    #[must_use]
    pub fn verification_code(&self) -> &str {
        self.stream.verification_code()
    }

    /// The codes matched: sends the account and the chosen vaults, then pins
    /// the new device (reachable on `sync_port` at the address it came from)
    /// and returns it. Convenience for [`Self::send_bundle`] +
    /// [`Self::wait_joined`] + `Session::pair_peer`.
    pub fn approve(
        mut self,
        session: &mut Session,
        secret_key: &password365_crypto::secret_key::AccountSecretKey,
        vaults: &[password365_model::VaultId],
        sync_port: u16,
    ) -> Result<Peer> {
        self.send_bundle(session, secret_key, vaults)?;
        let peer = self.wait_joined(sync_port)?;
        session.pair_peer(peer.clone())?;
        Ok(peer)
    }

    /// Sends the account and the chosen vaults (needs the session briefly).
    pub fn send_bundle(
        &mut self,
        session: &Session,
        secret_key: &password365_crypto::secret_key::AccountSecretKey,
        vaults: &[password365_model::VaultId],
    ) -> Result<()> {
        let bundle = session.join_bundle(secret_key, vaults)?;
        self.vaults = vaults.to_vec();
        self.send(&PairMessage::Bundle(bundle.to_vec()))
    }

    /// Waits (without the session: the user is typing their master password
    /// on the new device) until the new device reports who it is. The caller
    /// pins the returned peer with `Session::pair_peer`.
    pub fn wait_joined(mut self, sync_port: u16) -> Result<Peer> {
        match self.receive()? {
            PairMessage::Joined {
                device_id,
                name,
                static_public,
            } => {
                let mut peer = Peer::new(device_id, &name, static_public, Timestamp::now());
                peer.addr = Some(SocketAddr::new(self.from.ip(), sync_port).to_string());
                peer.vaults = Some(self.vaults);
                Ok(peer)
            }
            PairMessage::Cancelled => Err(NetError::Cancelled),
            PairMessage::Bundle(_) => {
                Err(VaultError::SyncProtocol("unexpected pairing message").into())
            }
        }
    }

    /// The codes did not match (or the user changed their mind).
    pub fn reject(mut self) -> Result<()> {
        self.send(&PairMessage::Cancelled)
    }

    fn send(&mut self, message: &PairMessage) -> Result<()> {
        self.stream.send(&message.encode()?)
    }

    fn receive(&mut self) -> Result<PairMessage> {
        Ok(PairMessage::decode(&self.stream.receive()?)?)
    }
}

/// The new device's side.
pub struct Joining {
    stream: SecureStream<TcpStream>,
    inviter: SocketAddr,
}

impl Joining {
    /// Connects to the device that shows `code`. Show the returned
    /// verification code and ask the user to compare it.
    pub fn connect(code: &str) -> Result<Self> {
        let invite = Invite::parse(code)?;
        let addr_text = invite.addr.clone().ok_or(NetError::NoAddress)?;
        let addr: SocketAddr = addr_text.parse().map_err(|_| InviteError::Malformed)?;
        let socket = TcpStream::connect_timeout(&addr, crate::tcp::TIMEOUT)?;
        prepare(&socket)?;
        // A throwaway key: the new device's real key is created with its store.
        let temporary = StaticKey::generate()?;
        let stream = SecureStream::establish(socket, Handshake::scanner(&temporary, &invite)?)?;
        Ok(Self {
            stream,
            inviter: addr,
        })
    }

    /// Six digits to compare with the other device's screen.
    #[must_use]
    pub fn verification_code(&self) -> &str {
        self.stream.verification_code()
    }

    /// Waits until the other device approves; returns the join bundle for
    /// `Store::join`.
    pub fn receive_bundle(&mut self) -> Result<Zeroizing<Vec<u8>>> {
        match PairMessage::decode(&self.stream.receive()?)? {
            PairMessage::Bundle(bundle) => Ok(Zeroizing::new(bundle)),
            PairMessage::Cancelled => Err(NetError::Cancelled),
            PairMessage::Joined { .. } => {
                Err(VaultError::SyncProtocol("unexpected pairing message").into())
            }
        }
    }

    /// Reports the new store's identity so the other device pins it, and
    /// remembers the other device as reachable on `sync_port`.
    pub fn complete(mut self, session: &mut Session, sync_port: u16) -> Result<()> {
        let joined = PairMessage::Joined {
            device_id: session.device_id(),
            name: session.device_name().to_owned(),
            static_public: session.static_key().public(),
        };
        self.stream.send(&joined.encode()?)?;
        if let Some(inviter) = session.peers().first().map(|p| p.device_id) {
            let addr = SocketAddr::new(self.inviter.ip(), sync_port);
            session.set_peer_addr(inviter, Some(addr.to_string()))?;
        }
        Ok(())
    }

    /// Gives up (codes did not match).
    pub fn cancel(mut self) -> Result<()> {
        self.stream.send(&PairMessage::Cancelled.encode()?)
    }
}

/// This computer's address on the local network, found without sending
/// anything (a UDP "connect" only picks the outgoing interface).
pub fn local_ip() -> Result<IpAddr> {
    let probe = UdpSocket::bind("0.0.0.0:0")?;
    // TEST-NET-1 (RFC 5737): never routed, nothing is sent.
    probe.connect("192.0.2.1:9")?;
    Ok(probe.local_addr()?.ip())
}

fn prepare(socket: &TcpStream) -> Result<()> {
    socket.set_read_timeout(Some(PAIRING_TIMEOUT))?;
    socket.set_write_timeout(Some(PAIRING_TIMEOUT))?;
    Ok(())
}
