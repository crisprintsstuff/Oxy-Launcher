use serde::{Deserialize, Serialize};

use crate::app_info::AppId;
use crate::error::{Error, Result};
use crate::version::Version;

const USER_AGENT: &str = "OxyStudio-Launcher/0.1.0";
const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub prerelease: bool,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> Result<Version> {
        Version::parse(&self.tag_name)
    }

    pub fn display_title(&self) -> &str {
        match &self.name {
            Some(n) if !n.trim().is_empty() => n.trim(),
            _ => &self.tag_name,
        }
    }
}

pub struct GitHubClient {
    agent: ureq::Agent,
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubClient {
    pub fn new() -> Self {
        let agent = ureq::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build();
        Self { agent }
    }

    pub fn fetch_releases(&self, repo: &str) -> Result<Vec<Release>> {
        let url = format!("{GITHUB_API_BASE}/repos/{repo}/releases");
        let resp = self
            .agent
            .get(&url)
            .set("Accept", "application/vnd.github.v3+json")
            .call()
            .map_err(|e| Error::Network(format!("failed to fetch releases for {repo}: {e}")))?;

        let releases: Vec<Release> = resp.into_json()?;
        Ok(releases)
    }

    pub fn fetch_latest_release(&self, repo: &str, allow_prerelease: bool) -> Result<Release> {
        let releases = self.fetch_releases(repo)?;
        for release in releases {
            if !allow_prerelease && release.prerelease {
                continue;
            }
            return Ok(release);
        }
        Err(Error::NoReleaseFound(repo.to_string()))
    }

    pub fn fetch_text(&self, url: &str) -> Result<String> {
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| Error::Network(format!("failed to download text from {url}: {e}")))?;
        resp.into_string()
            .map_err(|e| Error::Network(format!("failed to read response text: {e}")))
    }
}

/// Find the appropriate binary asset for an app from a release.
pub fn select_app_asset(release: &Release, app: AppId) -> Option<&Asset> {
    let mut matching: Vec<&Asset> = release
        .assets
        .iter()
        .filter(|asset| app.matches_current_platform(&asset.name))
        .collect();

    if matching.is_empty() {
        return None;
    }

    // Prioritize portable archives (.tar.gz, .zip) over standalone binaries or AppImages
    matching.sort_by_key(|a| {
        let name = a.name.to_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".zip") {
            0
        } else if name.ends_with(".appimage") || name.ends_with(".exe") {
            1
        } else {
            2
        }
    });

    matching.into_iter().next()
}

/// Find the checksums file in a release (e.g. SHA256SUMS.txt or SHA256SUMS-<os>.txt).
pub fn find_checksum_asset(release: &Release) -> Option<&Asset> {
    let os = std::env::consts::OS;
    let os_specific = format!("sha256sums-{os}.txt");

    // Prefer os-specific checksum if present, otherwise general SHA256SUMS.txt
    release
        .assets
        .iter()
        .find(|a| a.name.to_lowercase() == os_specific)
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("sha256sums.txt"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_json() {
        let raw = r#"{
            "tag_name": "v0.4.0-alpha",
            "name": "OxyBrush 0.4.0 Alpha",
            "body": "First public preview",
            "prerelease": true,
            "assets": [
                {
                    "name": "oxybrush-0.4.0-alpha-linux-x86_64.tar.gz",
                    "size": 15000000,
                    "browser_download_url": "https://example.com/oxybrush.tar.gz"
                },
                {
                    "name": "SHA256SUMS.txt",
                    "size": 120,
                    "browser_download_url": "https://example.com/SHA256SUMS.txt"
                }
            ]
        }"#;

        let release: Release = serde_json::from_str(raw).unwrap();
        assert_eq!(release.version().unwrap().to_string(), "0.4.0-alpha");
        assert!(release.prerelease);
        assert_eq!(release.assets.len(), 2);

        let checksum_asset = find_checksum_asset(&release);
        assert!(checksum_asset.is_some());
        assert_eq!(checksum_asset.unwrap().name, "SHA256SUMS.txt");
    }
}
