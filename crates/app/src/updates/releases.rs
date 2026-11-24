//! Compare the installed build to the official stable Riina release.
use crate::http::{RequestPolicy, agent_config};
use semver::Version;
use serde::Deserialize;
use std::{cmp::Ordering, time::Duration};

const LATEST: &str = "https://api.github.com/repos/deminden/riina/releases/latest";
const TAG_PAGE: &str = "https://github.com/deminden/riina/releases/tag/";
const METADATA_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct Record {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

pub(super) struct Release {
    pub(super) version: Version,
    pub(super) url: String,
}

pub(super) enum Outcome {
    Available(Release),
    Current(Version),
}

pub(super) fn check() -> Result<Outcome, String> {
    let installed = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| error.to_string())?;
    let mut response = agent_config(
        Duration::from_secs(12),
        RequestPolicy::HttpsWithoutRedirects,
    )
    .new_agent()
    .get(LATEST)
    .header("Accept", "application/vnd.github+json")
    .call()
    .map_err(|error| {
        crate::http::format_api_error(
            "GitHub",
            "release check",
            error,
            &[
                (403, "GitHub denied the request. Try again later."),
                (429, "GitHub rate limit reached. Try again later."),
            ],
        )
    })?;
    let data = response
        .body_mut()
        .with_config()
        .limit(METADATA_LIMIT)
        .read_to_vec()
        .map_err(|error| format!("Could not read release information: {error}"))?;
    compare(&data, &installed)
}

fn compare(data: &[u8], installed: &Version) -> Result<Outcome, String> {
    let record: Record = serde_json::from_slice(data)
        .map_err(|_| "GitHub returned invalid release information.".to_owned())?;
    let tag = &record.tag_name;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| "The published release tag is not a supported version.".to_owned())?;
    if record.draft || record.prerelease || !version.pre.is_empty() {
        return Err("GitHub did not return a stable release.".into());
    }
    Ok(if version.cmp_precedence(installed) == Ordering::Greater {
        // Validated SemVer limits the path segment. Only open the project release page.
        Outcome::Available(Release {
            version,
            url: format!("{TAG_PAGE}{tag}"),
        })
    } else {
        Outcome::Current(version)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(tag: &str, draft: bool, prerelease: bool) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "tag_name": tag,
            "draft": draft,
            "prerelease": prerelease,
            "html_url": "https://example.invalid/elsewhere"
        }))
        .unwrap()
    }

    #[test]
    fn compares_version_precedence_and_only_links_to_the_project() {
        let installed = Version::parse("4.2.9").unwrap();
        let Outcome::Available(release) =
            compare(&record("v4.2.10", false, false), &installed).unwrap()
        else {
            panic!("4.2.10 should follow 4.2.9");
        };
        assert_eq!(release.version, Version::new(4, 2, 10));
        assert_eq!(release.url, format!("{TAG_PAGE}v4.2.10"));
        for tag in ["v4.2.8", "4.2.9+build.2"] {
            assert!(matches!(
                compare(&record(tag, false, false), &installed),
                Ok(Outcome::Current(_))
            ));
        }
    }

    #[test]
    fn rejects_drafts_prereleases_and_malformed_tags() {
        let installed = Version::new(4, 2, 1);
        for (tag, draft, beta) in [
            ("v5.0.0", true, false),
            ("v5.0.0", false, true),
            ("v5.0.0-beta.1", false, false),
            ("v5.0.0/other", false, false),
        ] {
            assert!(compare(&record(tag, draft, beta), &installed).is_err());
        }
    }
}
