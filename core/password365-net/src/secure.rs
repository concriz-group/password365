//! A Noise channel carried over any byte stream (TCP in practice).

use std::io::{Read, Write};

use password365_sync::pairing::{Channel, Handshake};
use zeroize::Zeroizing;

use crate::{Result, frame};

/// An encrypted, authenticated stream.
pub struct SecureStream<S> {
    stream: S,
    channel: Channel,
}

impl<S: Read + Write> SecureStream<S> {
    /// Runs `handshake` over `stream` (taking turns as the pattern requires).
    pub fn establish(mut stream: S, mut handshake: Handshake) -> Result<Self> {
        while !handshake.is_finished() {
            if handshake.is_my_turn() {
                frame::write(&mut stream, &handshake.write()?)?;
            } else {
                handshake.read(&frame::read(&mut stream)?)?;
            }
        }
        Ok(Self {
            stream,
            channel: handshake.finish()?,
        })
    }

    /// The peer's static key, as proven by the handshake.
    #[must_use]
    pub fn remote_static(&self) -> [u8; 32] {
        self.channel.remote_static()
    }

    /// The code both screens show while pairing.
    #[must_use]
    pub fn verification_code(&self) -> &str {
        self.channel.verification_code()
    }

    /// Sends one message.
    pub fn send(&mut self, plaintext: &[u8]) -> Result<()> {
        let sealed = self.channel.seal(plaintext)?;
        Ok(frame::write(&mut self.stream, &sealed)?)
    }

    /// Receives one message.
    pub fn receive(&mut self) -> Result<Zeroizing<Vec<u8>>> {
        let sealed = frame::read(&mut self.stream)?;
        Ok(self.channel.open(&sealed)?)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use password365_sync::DeviceId;
    use password365_sync::pairing::{Invite, StaticKey};

    use super::*;

    #[test]
    fn pairing_over_a_socket_shows_the_same_code_on_both_sides() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let inviter_key = StaticKey::generate().unwrap();
        let invite = Invite::new(DeviceId::new(), inviter_key.public(), None);
        let scanned = Invite::parse(&invite.to_code()).unwrap();
        let addr = listener.local_addr().unwrap();
        let inviter = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut secure =
                SecureStream::establish(socket, Handshake::inviter(&inviter_key, &invite).unwrap())
                    .unwrap();
            let got = secure.receive().unwrap().to_vec();
            secure.send(b"welcome").unwrap();
            (secure.verification_code().to_owned(), got)
        });
        let socket = TcpStream::connect(addr).unwrap();
        let mut joiner = SecureStream::establish(
            socket,
            Handshake::scanner(&StaticKey::generate().unwrap(), &scanned).unwrap(),
        )
        .unwrap();
        joiner.send(b"hello").unwrap();
        assert_eq!(joiner.receive().unwrap().as_slice(), b"welcome");
        let (code, got) = inviter.join().unwrap();
        assert_eq!(code, joiner.verification_code());
        assert_eq!(got, b"hello");
        assert_eq!(joiner.remote_static(), scanned.static_public);
    }
}
