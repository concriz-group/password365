//! Parsed website identity.

use url::{Host, Url};

/// The parts of a URL that matter for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// `http` or `https`.
    pub scheme: String,
    /// Lower-case host name or IP address.
    pub host: String,
    /// Explicit non-default port.
    pub port: Option<u16>,
    /// Registrable domain (eTLD+1); for IPs and single-label hosts, the host itself.
    pub registrable: String,
}

impl Site {
    /// Parses a URL. Values without a scheme (`github.com/login`) are read as
    /// `https://`. Only `http` and `https` are accepted.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Only text without any scheme ("github.com/login") gets an implied https://.
        // Anything with a scheme (mailto:, android:, ftp:) is parsed as-is and rejected below.
        let url = match Url::parse(trimmed) {
            Ok(url) => url,
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                Url::parse(&format!("https://{trimmed}")).ok()?
            }
            Err(_) => return None,
        };
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        let host = url.host()?;
        let host_text = match &host {
            Host::Domain(d) => d.trim_end_matches('.').to_ascii_lowercase(),
            Host::Ipv4(ip) => ip.to_string(),
            Host::Ipv6(ip) => ip.to_string(),
        };
        let registrable = match host {
            Host::Domain(_) => registrable_domain(&host_text),
            _ => host_text.clone(),
        };
        Some(Self {
            scheme: url.scheme().to_owned(),
            host: host_text,
            port: url.port(),
            registrable,
        })
    }

    /// `scheme://host[:port]`.
    #[must_use]
    pub fn origin(&self) -> String {
        match self.port {
            Some(port) => format!("{}://{}:{port}", self.scheme, self.host),
            None => format!("{}://{}", self.scheme, self.host),
        }
    }
}

/// eTLD+1 of a domain, or the domain itself when it has no public suffix
/// (single-label names such as `localhost`, or a bare suffix such as `co.uk`).
fn registrable_domain(host: &str) -> String {
    if !host.contains('.') {
        return host.to_owned();
    }
    psl::domain_str(host).unwrap_or(host).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(raw: &str) -> Site {
        Site::parse(raw).unwrap()
    }

    #[test]
    fn parses_full_urls() {
        let s = site("https://Accounts.Google.com:8443/login?x=1");
        assert_eq!(s.scheme, "https");
        assert_eq!(s.host, "accounts.google.com");
        assert_eq!(s.port, Some(8443));
        assert_eq!(s.registrable, "google.com");
        assert_eq!(s.origin(), "https://accounts.google.com:8443");
    }

    #[test]
    fn bare_hosts_default_to_https() {
        let s = site("github.com/login");
        assert_eq!(
            (s.scheme.as_str(), s.host.as_str()),
            ("https", "github.com")
        );
        assert_eq!(s.origin(), "https://github.com");
    }

    #[test]
    fn multi_part_public_suffixes() {
        assert_eq!(site("https://www.bbc.co.uk").registrable, "bbc.co.uk");
        assert_eq!(site("https://user.github.io").registrable, "user.github.io");
    }

    #[test]
    fn ips_and_single_label_hosts_are_their_own_site() {
        assert_eq!(site("http://192.168.1.1/admin").registrable, "192.168.1.1");
        assert_eq!(site("http://[::1]:3000").registrable, "::1");
        assert_eq!(site("http://localhost:8080").registrable, "localhost");
    }

    #[test]
    fn trailing_dot_is_ignored() {
        assert_eq!(site("https://example.com.").host, "example.com");
    }

    #[test]
    fn rejects_non_web_and_garbage() {
        for raw in [
            "",
            "   ",
            "ftp://example.com",
            "android://com.app",
            "mailto:a@b.c",
            "http://",
            "https://user:pw@",
        ] {
            assert!(Site::parse(raw).is_none(), "{raw}");
        }
    }
}
