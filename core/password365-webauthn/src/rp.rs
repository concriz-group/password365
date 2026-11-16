//! Which relying-party ids a page may use (`WebAuthn` §5.1.3/§5.1.4): its own
//! host, or a parent domain that is not a public suffix (`login.example.com`
//! may use `example.com`, never `com` or `co.uk`). Only secure origins:
//! `https`, or `http://localhost` for development.

use password365_match::Site;

use crate::{Result, WebAuthnError};

/// The page's origin (`https://host[:port]`) if it may use `rp_id`.
pub fn check(page_url: &str, rp_id: &str) -> Result<String> {
    let wrong = || WebAuthnError::WrongSite(rp_id.to_owned());
    let page = Site::parse(page_url).ok_or_else(wrong)?;
    let secure =
        page.scheme == "https" || (page.host == "localhost" || page.host.ends_with(".localhost"));
    if !secure {
        return Err(wrong());
    }
    let rp_id = rp_id.trim().to_ascii_lowercase();
    let rp = Site::parse(&format!("https://{rp_id}")).ok_or_else(wrong)?;
    let same = page.host == rp.host;
    // A parent domain must still be inside the page's registrable domain (a
    // public suffix is its own "registrable" fallback, so it never matches).
    let parent =
        page.host.ends_with(&format!(".{}", rp.host)) && rp.registrable == page.registrable;
    if rp.host != rp_id || !(same || parent) {
        return Err(wrong());
    }
    Ok(page.origin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_may_use_its_host_or_a_parent_domain() {
        assert_eq!(
            check("https://example.com/login", "example.com").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            check("https://login.example.com", "example.com").unwrap(),
            "https://login.example.com"
        );
        assert_eq!(
            check("https://a.b.example.co.uk:8443/x", "b.example.co.uk").unwrap(),
            "https://a.b.example.co.uk:8443"
        );
        assert!(check("http://localhost:3000", "localhost").is_ok());
    }

    #[test]
    fn other_sites_public_suffixes_and_insecure_pages_are_refused() {
        for (page, rp) in [
            ("https://evil.com", "example.com"),
            ("https://example.com.evil.com", "example.com"),
            ("https://notexample.com", "example.com"),
            ("https://example.com", "login.example.com"),
            ("https://example.co.uk", "co.uk"),
            ("https://example.com", "com"),
            ("http://example.com", "example.com"),
            ("https://user.github.io", "github.io"),
            ("not a url", "example.com"),
            ("https://example.com", "example.com/path"),
        ] {
            assert!(
                matches!(check(page, rp), Err(WebAuthnError::WrongSite(_))),
                "{page} → {rp}"
            );
        }
    }
}
