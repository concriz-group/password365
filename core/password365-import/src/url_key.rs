//! Normalized URL keys used to compare logins and derive titles.

use url::Url;

/// Normalizes a URL to `scheme://host[:port]` (lowercase) for comparisons.
/// Unparseable input falls back to its trimmed lowercase form.
#[must_use]
pub fn origin_key(raw: &str) -> String {
    let trimmed = raw.trim();
    match Url::parse(trimmed) {
        // Opaque origins (android://, custom schemes) all serialize as "null",
        // so they must fall back to the raw text or they would all collide.
        Ok(url) if url.origin().is_tuple() => url.origin().ascii_serialization(),
        _ => trimmed.to_lowercase(),
    }
}

/// Host name of a URL without a leading `www.`, if it has one.
#[must_use]
pub fn display_host(raw: &str) -> Option<String> {
    let url = Url::parse(raw.trim()).ok()?;
    let host = url.host_str()?;
    Some(host.strip_prefix("www.").unwrap_or(host).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_ignores_path_query_and_case() {
        assert_eq!(
            origin_key("https://GitHub.com/login?x=1"),
            "https://github.com"
        );
        assert_eq!(origin_key(" https://github.com/ "), "https://github.com");
    }

    #[test]
    fn origin_keeps_non_default_port_and_scheme() {
        assert_eq!(
            origin_key("http://localhost:8080/a"),
            "http://localhost:8080"
        );
        assert_ne!(origin_key("http://a.com"), origin_key("https://a.com"));
    }

    #[test]
    fn origin_falls_back_for_non_urls() {
        assert_eq!(origin_key(" Android://App "), "android://app");
        assert_eq!(origin_key("not a url"), "not a url");
        assert_ne!(
            origin_key("android://app.one"),
            origin_key("android://app.two")
        );
    }

    #[test]
    fn display_host_strips_www() {
        assert_eq!(
            display_host("https://www.example.com/x").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            display_host("https://mail.example.com").as_deref(),
            Some("mail.example.com")
        );
        assert_eq!(display_host("garbage"), None);
        assert_eq!(display_host("mailto:a@b.c"), None);
    }
}
