//! Finding paired devices on the local network when their address changed
//! (mDNS / Bonjour, service type `_password365._tcp`).
//!
//! Privacy: the advertisement carries only a random name, chosen anew every
//! time advertising starts, and the port. There is no device name, account
//! or key in it. Who is behind an address is learnt only through the
//! encrypted handshake, and only paired devices are accepted.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::Result;

/// mDNS service type.
pub const SERVICE_TYPE: &str = "_password365._tcp.local.";
/// How long a lookup listens for answers.
pub const BROWSE_TIME: Duration = Duration::from_secs(3);

/// A running advertisement; stops when dropped.
pub struct Advertiser {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertiser {
    /// Announces this device on `ip`:`port` under a fresh random name.
    pub fn start(ip: IpAddr, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| mdns_error(&e))?;
        let name = random_name()?;
        let host = format!("{name}.local.");
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &name,
            &host,
            ip,
            port,
            None::<std::collections::HashMap<String, String>>,
        )
        .map_err(|e| mdns_error(&e))?;
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(|e| mdns_error(&e))?;
        Ok(Self { daemon, fullname })
    }

    /// The advertised instance name (random).
    #[must_use]
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Addresses of Password365 devices answering on the local network within `wait`.
pub fn browse(wait: Duration) -> Result<Vec<SocketAddr>> {
    let daemon = ServiceDaemon::new().map_err(|e| mdns_error(&e))?;
    let events = daemon.browse(SERVICE_TYPE).map_err(|e| mdns_error(&e))?;
    let deadline = Instant::now() + wait;
    let mut found = BTreeSet::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match events.recv_timeout(left) {
            Ok(ServiceEvent::ServiceResolved(service)) => {
                let port = service.get_port();
                found.extend(
                    service
                        .get_addresses_v4()
                        .into_iter()
                        .map(|ip| SocketAddr::new(ip.into(), port)),
                );
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.shutdown();
    Ok(found.into_iter().collect())
}

fn random_name() -> Result<String> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(format!(
        "password365-{}",
        bytes.iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
    ))
}

fn mdns_error(e: &mdns_sd::Error) -> crate::NetError {
    std::io::Error::other(format!("local network discovery: {e}")).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_random_and_carry_nothing_about_the_device() {
        let (a, b) = (random_name().unwrap(), random_name().unwrap());
        assert_ne!(a, b);
        assert!(a.starts_with("password365-") && a.len() == "password365-".len() + 16);
    }

    #[test]
    fn errors_are_reported_as_connection_problems() {
        let e = mdns_error(&mdns_sd::Error::Msg("boom".into()));
        assert!(e.to_string().contains("local network discovery: boom"));
    }

    /// Needs multicast on a real network interface; run with `--ignored`.
    #[test]
    #[ignore = "needs multicast networking"]
    fn an_advertised_device_is_found() {
        let ip = crate::pairing::local_ip().unwrap();
        let advertiser = Advertiser::start(ip, 47_999).unwrap();
        assert!(advertiser.fullname().starts_with("password365-"));
        let found = browse(BROWSE_TIME).unwrap();
        assert!(
            found.contains(&SocketAddr::new(ip, 47_999)),
            "found {found:?}"
        );
    }
}
