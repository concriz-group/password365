//! How closely a saved URL matches the current page, and whether filling is allowed.

use crate::Site;

/// Strength of a match, from strongest to weakest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchLevel {
    /// Same scheme, host and port.
    Origin,
    /// Same host (different scheme or port).
    Host,
    /// Same registrable domain (e.g. `login.example.com` vs `www.example.com`).
    Site,
}

/// How `saved` matches `page`, or `None` if they are different sites.
#[must_use]
pub fn match_level(page: &Site, saved: &Site) -> Option<MatchLevel> {
    if page.registrable != saved.registrable {
        return None;
    }
    if page.host != saved.host {
        return Some(MatchLevel::Site);
    }
    if page.origin() == saved.origin() {
        Some(MatchLevel::Origin)
    } else {
        Some(MatchLevel::Host)
    }
}

/// Whether credentials saved for `saved` may be filled into `page`.
///
/// Requires a match and refuses to downgrade: a login saved for an `https`
/// site is never offered on an `http` page.
#[must_use]
pub fn fill_allowed(page: &Site, saved: &Site) -> bool {
    let downgrade = saved.scheme == "https" && page.scheme == "http";
    match_level(page, saved).is_some() && !downgrade
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(page: &str, saved: &str) -> Option<MatchLevel> {
        match_level(&Site::parse(page).unwrap(), &Site::parse(saved).unwrap())
    }

    fn allowed(page: &str, saved: &str) -> bool {
        fill_allowed(&Site::parse(page).unwrap(), &Site::parse(saved).unwrap())
    }

    #[test]
    fn levels() {
        assert_eq!(
            level("https://github.com/login", "https://github.com"),
            Some(MatchLevel::Origin)
        );
        assert_eq!(
            level("https://github.com:8443", "https://github.com"),
            Some(MatchLevel::Host)
        );
        assert_eq!(
            level("https://gist.github.com", "https://github.com"),
            Some(MatchLevel::Site)
        );
        assert_eq!(level("https://gitlab.com", "https://github.com"), None);
    }

    #[test]
    fn ordering_puts_strongest_first() {
        assert!(MatchLevel::Origin < MatchLevel::Host && MatchLevel::Host < MatchLevel::Site);
    }

    #[test]
    fn phishing_lookalikes_do_not_match() {
        assert_eq!(
            level("https://github.com.evil.io", "https://github.com"),
            None
        );
        assert_eq!(level("https://evil-github.com", "https://github.com"), None);
        assert_eq!(level("https://githuh.com", "https://github.com"), None);
    }

    #[test]
    fn shared_hosting_suffixes_are_separate_sites() {
        // github.io is a public suffix: alice and bob are different sites.
        assert_eq!(
            level("https://alice.github.io", "https://bob.github.io"),
            None
        );
    }

    #[test]
    fn fill_requires_match_and_no_downgrade() {
        assert!(allowed(
            "https://www.example.com/login",
            "https://example.com"
        ));
        assert!(
            allowed("https://example.com", "http://example.com"),
            "upgrade is fine"
        );
        assert!(
            !allowed("http://example.com", "https://example.com"),
            "downgrade refused"
        );
        assert!(!allowed("https://example.org", "https://example.com"));
    }

    #[test]
    fn ip_addresses_need_exact_host() {
        assert!(allowed("http://192.168.1.1/x", "http://192.168.1.1"));
        assert!(!allowed("http://192.168.1.2", "http://192.168.1.1"));
    }
}
