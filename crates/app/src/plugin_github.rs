//! GitHub source resolution for IINA-compatible plugin packages.

use std::time::Duration;
use ureq::Agent;
use url::Url;

const MAX_RELEASE_METADATA: u64 = 2 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_UPDATE_METADATA: u64 = 2 * 1024 * 1024;

#[derive(serde::Deserialize)]
struct Release {
    assets: Vec<ReleaseAsset>,
}

#[derive(serde::Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct PluginMetadata {
    #[serde(rename = "ghVersion")]
    github_version: u64,
    version: String,
}

pub(crate) fn stage_repository(
    source: &str,
    plugins_directory: &std::path::Path,
) -> Result<crate::plugin_packages::StagedPackage, String> {
    let (owner, repository) = standardize_repository(source)?;
    let agent = agent(Duration::from_secs(45));
    let release_url = format!("https://api.github.com/repos/{owner}/{repository}/releases/latest");

    let release_package = if let Ok(mut response) = agent
        .get(&release_url)
        .header("User-Agent", "Riina/0.3.0")
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        if let Ok(metadata) = response
            .body_mut()
            .with_config()
            .limit(MAX_RELEASE_METADATA)
            .read_to_vec()
            && let Ok(release) = serde_json::from_slice::<Release>(&metadata)
        {
            if let Some(asset) = release
                .assets
                .into_iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with(".iinaplgz"))
                && is_github_download(&asset.browser_download_url)
            {
                download(&agent, &asset.browser_download_url).and_then(|bytes| {
                    crate::plugin_packages::stage_archive_bytes(bytes, plugins_directory)
                })
            } else {
                Err("The latest GitHub release has no plugin package asset.".into())
            }
        } else {
            Err("Could not read the latest GitHub release metadata.".into())
        }
    } else {
        Err("Could not reach the latest GitHub release.".into())
    };
    if let Ok(staged) = release_package {
        return Ok(staged);
    }

    let source_url =
        format!("https://codeload.github.com/{owner}/{repository}/zip/refs/heads/main");
    let source_result = download(&agent, &source_url)
        .and_then(|bytes| crate::plugin_packages::stage_archive_bytes(bytes, plugins_directory));
    source_result.map_err(|source_error| {
        format!("GitHub release package failed and source archive was unavailable: {source_error}")
    })
}

pub(crate) fn check_new_version(
    repository: &str,
    installed_github_version: u64,
) -> Result<Option<String>, String> {
    let (owner, repository) = standardize_repository(repository)?;
    let url = format!("https://raw.githubusercontent.com/{owner}/{repository}/main/Info.json");
    let mut response = agent(Duration::from_secs(15))
        .get(&url)
        .header("User-Agent", "Riina/0.3.0")
        .call()
        .map_err(|error| format!("Could not check plugin version: {error}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_UPDATE_METADATA)
        .read_to_vec()
        .map_err(|error| format!("Could not read plugin version: {error}"))?;
    let current: PluginMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid plugin version metadata: {error}"))?;
    Ok((current.github_version > installed_github_version).then_some(current.version))
}

fn agent(timeout: Duration) -> Agent {
    Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

fn download(agent: &Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent
        .get(url)
        .header("User-Agent", "Riina/0.3.0")
        .call()
        .map_err(|error| format!("GitHub download failed: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read GitHub plugin archive: {error}"))
}

fn standardize_repository(source: &str) -> Result<(String, String), String> {
    let source = source.trim();
    let path = if source.starts_with("https://") {
        let url = Url::parse(source).map_err(|_| invalid_repository())?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid_repository());
        }
        url.path().trim_matches('/').to_owned()
    } else {
        source.to_owned()
    };
    let mut parts = path.split('/');
    let owner = parts.next().unwrap_or_default();
    let repository = parts.next().unwrap_or_default();
    if parts.next().is_some() || !valid_component(owner) || !valid_component(repository) {
        return Err(invalid_repository());
    }
    Ok((owner.to_owned(), repository.to_owned()))
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_github_download(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn invalid_repository() -> String {
    "Enter a GitHub repository as owner/repo or https://github.com/owner/repo.".into()
}
