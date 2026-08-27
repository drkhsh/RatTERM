use std::time::Duration;

use semver::Version;
use serde::Deserialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// Newest published release whose tag starts with `tag_prefix`, e.g. `("mkrueger/icy_tools", "IcyTerm")`.
///
/// Returns `None` on any network or parse failure so callers fall back to their own version.
pub fn latest_release(repo: &str, tag_prefix: &str) -> Option<Version> {
    let response = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("icy_tools/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?
        .get(format!("https://api.github.com/repos/{repo}/releases?per_page=100"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .ok()?
        .error_for_status()
        .ok()?;

    // GitHub lists releases newest first, so the first prefix match is the latest.
    response
        .json::<Vec<Release>>()
        .ok()?
        .into_iter()
        .find_map(|release| release.tag_name.strip_prefix(tag_prefix).and_then(|version| Version::parse(version).ok()))
}
