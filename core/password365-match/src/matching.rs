//! How closely a saved URL matches the current page, and whether filling is allowed.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

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

/// The sites that are one account under several domains (`templates/equivalent-domains.json`).
fn equivalent_groups() -> &'static HashMap<String, usize> {
    static GROUPS: OnceLock<HashMap<String, usize>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        #[derive(Deserialize)]
        struct File {
            groups: Vec<Vec<String>>,
        }
        let file: File =
            serde_json::from_str(include_str!("../../../templates/equivalent-domains.json"))
                .unwrap_or(File { groups: Vec::new() });
        file.groups
            .into_iter()
            .enumerate()
            .flat_map(|(index, domains)| {
                domains
                    .into_iter()
                    .map(move |d| (d.to_ascii_lowercase(), index))
            })
            .collect()
    })
}

/// Whether two registrable domains are listed as the same account.
fn equivalent(a: &str, b: &str) -> bool {
    let groups = equivalent_groups();
    matches!((groups.get(a), groups.get(b)), (Some(x), Some(y)) if x == y)
}

/// How `saved` matches `page`, or `None` if they are different sites.
#[must_use]
pub fn match_level(page: &Site, saved: &Site) -> Option<MatchLevel> {
    if saved.wildcard && !page.wildcard {
        // `amazon.*` saved by the person: the page's name before its ending (`amazon` in
        // `amazon.co.uk`) must be exactly that, with a real top-level domain after it.
        let name = page.registrable.split('.').next().unwrap_or_default();
        return (page.registrable.contains('.') && name == saved.registrable)
            .then_some(MatchLevel::Site);
    }
    if page.registrable != saved.registrable {
        // A listed pair (`amazon.com` and `amazon.es`) is the same account, at the weakest level.
        return equivalent(&page.registrable, &saved.registrable).then_some(MatchLevel::Site);
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
    fn a_brands_country_sites_are_one_account_but_look_alikes_are_not() {
        assert_eq!(
            level("https://www.amazon.es/ap/signin", "https://amazon.com"),
            Some(MatchLevel::Site)
        );
        assert_eq!(
            level("https://accounts.google.com", "https://youtube.com"),
            Some(MatchLevel::Site)
        );
        assert!(allowed("https://www.amazon.de/", "https://www.amazon.com/"));
        // Not listed: another top-level domain is not assumed to be the brand's.
        assert_eq!(level("https://amazon.xyz", "https://amazon.com"), None);
        assert_eq!(
            level("https://amazon-login.example", "https://amazon.com"),
            None
        );
        // Different groups stay apart.
        assert_eq!(level("https://amazon.com", "https://google.com"), None);
        // No downgrade from https to http, even between listed sites.
        assert!(!allowed("http://amazon.es", "https://amazon.com"));
    }

    #[test]
    fn a_wildcard_address_saved_by_the_person_matches_that_name_in_any_domain() {
        for page in [
            "https://www.amazon.es/ap/signin",
            "https://amazon.co.uk",
            "https://amazon.xyz",
        ] {
            assert_eq!(level(page, "amazon.*"), Some(MatchLevel::Site), "{page}");
        }
        assert!(allowed("https://amazon.de", "https://www.amazon.*/login"));
        // Only the exact name: look-alikes and other names stay out.
        for page in [
            "https://amazon-login.com",
            "https://notamazon.com",
            "https://amazon.evil.com",
            "https://amazonia.es",
        ] {
            assert_eq!(level(page, "amazon.*"), None, "{page}");
        }
        // Not a wildcard: too broad or malformed.
        for bad in ["*", "*.com", "*.*", "a*.es", "amazon.*.com", "co.uk.*"] {
            assert!(Site::parse(bad).is_none_or(|s| !s.wildcard), "{bad}");
        }
        // A plain host on a single label (no ending) is not matched by a wildcard.
        assert_eq!(level("http://amazon", "amazon.*"), None);
        // No downgrade from https to http.
        assert!(!allowed("http://amazon.es", "https://amazon.*"));
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

    #[test]
    fn android_apps_match_only_their_own_package() {
        assert_eq!(
            level(
                "androidapp://com.example.bank",
                "androidapp://com.example.bank"
            ),
            Some(MatchLevel::Origin)
        );
        assert!(allowed(
            "androidapp://com.example.bank",
            "androidapp://COM.example.bank"
        ));
        assert_eq!(
            level(
                "androidapp://com.example.bank",
                "androidapp://com.example.bank.evil"
            ),
            None
        );
        assert_eq!(
            level(
                "androidapp://com.example.bank",
                "androidapp://com.other.bank"
            ),
            None
        );
        // An app is not a website, and a website is not an app.
        assert_eq!(
            level("androidapp://com.example.bank", "https://example.bank"),
            None
        );
        assert_eq!(
            level("https://bank.example.com", "androidapp://com.example.bank"),
            None
        );
    }

    #[test]
    fn android_app_identities_have_no_path() {
        assert!(Site::parse("androidapp://com.example.bank/login").is_none());
        assert!(Site::parse("androidapp://").is_none());
    }
}
