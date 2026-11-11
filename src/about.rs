//! Application identity shown on the About page.
//!
//! Cargo already stores the version, author, license and repository, so the
//! interface reads them from the build instead of keeping a second copy that
//! could drift. `VERSION` is the value an update check compares with the
//! latest GitHub release tag, and `RELEASES_URL` is where that check looks.

/// Product name, shared by the window, the desktop entry and the About page.
pub const NAME: &str = "Legion Monitor";

/// Version from `Cargo.toml`; an update check compares release tags to this.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Authors from `Cargo.toml`. Cargo separates multiple names with `:`.
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

/// SPDX license identifier from `Cargo.toml`.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// Project page. The releases URL below is derived from it.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Releases page — where newer versions and their notes are published.
pub const RELEASES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");

/// Author names as a readable list ("A, B"), or "—" when Cargo has none.
pub fn display_authors() -> String {
    let names: Vec<&str> = AUTHORS
        .split(':')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        "—".to_string()
    } else {
        names.join(", ")
    }
}

/// Repository without its scheme, short enough for a row in the About card.
pub fn display_repository() -> &'static str {
    REPOSITORY
        .strip_prefix("https://")
        .or_else(|| REPOSITORY.strip_prefix("http://"))
        .unwrap_or(REPOSITORY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_looks_like_a_release() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(parts.len() >= 2, "unexpected version {VERSION:?}");
        assert!(
            parts.iter().all(|part| !part.is_empty()),
            "unexpected version {VERSION:?}"
        );
    }

    #[test]
    fn author_list_is_readable() {
        assert!(!display_authors().is_empty());
        assert!(!display_authors().contains(':'));
    }

    #[test]
    fn repository_has_no_scheme_when_displayed() {
        assert!(display_repository().starts_with("github.com/"));
        assert!(RELEASES_URL.ends_with("/releases"));
    }
}
