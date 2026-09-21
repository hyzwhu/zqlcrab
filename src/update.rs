//! Application update checker querying GitHub Releases API.

use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate {
        current_version: String,
        latest_version: String,
        checked_time: String,
    },
    NewVersionAvailable {
        current_version: String,
        latest_version: String,
        release_name: Option<String>,
        release_url: String,
    },
    Failed {
        error: String,
    },
}

/// Parse a version string like "v0.1.1" or "0.1.1" into (major, minor, patch).
pub fn parse_semver(version_str: &str) -> Option<(u32, u32, u32)> {
    let clean = version_str
        .trim()
        .trim_start_matches(|c| c == 'v' || c == 'V');
    let core = clean.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    Some((major, minor, patch))
}

/// Returns true if `latest_tag` is strictly newer than `current_pkg_ver`.
pub fn is_newer_version(latest_tag: &str, current_pkg_ver: &str) -> bool {
    if let (Some(latest), Some(current)) = (parse_semver(latest_tag), parse_semver(current_pkg_ver))
    {
        latest > current
    } else {
        let l_clean = latest_tag
            .trim()
            .trim_start_matches(|c| c == 'v' || c == 'V');
        let c_clean = current_pkg_ver
            .trim()
            .trim_start_matches(|c| c == 'v' || c == 'V');
        !l_clean.is_empty() && l_clean != c_clean
    }
}

/// Query the latest public release from GitHub Releases API
pub async fn check_for_updates(current_version: &str) -> UpdateCheckResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return UpdateCheckResult::Failed {
                error: format!("HTTP client initialization failed: {e}"),
            };
        }
    };

    let url = "https://api.github.com/repos/hyzwhu/zqlcrab/releases/latest";
    let user_agent = format!("zqlcrab/{}", current_version);

    let res = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .send()
        .await;

    let response = match res {
        Ok(r) => r,
        Err(e) => {
            let err_str = if e.is_timeout() {
                "Connection timed out while checking GitHub Releases".to_string()
            } else if e.is_connect() {
                "Could not connect to GitHub. Please check your internet connection.".to_string()
            } else {
                format!("Failed to reach GitHub: {e}")
            };
            return UpdateCheckResult::Failed { error: err_str };
        }
    };

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return UpdateCheckResult::Failed {
            error: "GitHub API rate limit exceeded. Please try again later.".to_string(),
        };
    }

    if !response.status().is_success() {
        return UpdateCheckResult::Failed {
            error: format!("GitHub API returned HTTP {}", response.status()),
        };
    }

    let release: GithubRelease = match response.json().await {
        Ok(rel) => rel,
        Err(e) => {
            return UpdateCheckResult::Failed {
                error: format!("Failed to parse GitHub release response: {e}"),
            };
        }
    };

    let latest_ver = release
        .tag_name
        .trim_start_matches(|c| c == 'v' || c == 'V')
        .to_string();
    let current_ver = current_version
        .trim_start_matches(|c| c == 'v' || c == 'V')
        .to_string();

    if is_newer_version(&release.tag_name, current_version) {
        UpdateCheckResult::NewVersionAvailable {
            current_version: current_ver,
            latest_version: latest_ver,
            release_name: release.name,
            release_url: release.html_url,
        }
    } else {
        let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
        UpdateCheckResult::UpToDate {
            current_version: current_ver,
            latest_version: latest_ver,
            checked_time: now_str,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        assert_eq!(parse_semver("v0.1.1"), Some((0, 1, 1)));
        assert_eq!(parse_semver("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver("v1.20.3-beta.1"), Some((1, 20, 3)));
        assert_eq!(parse_semver("2.0"), Some((2, 0, 0)));
        assert_eq!(parse_semver("invalid"), None);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.1.1", "0.1.0"));
        assert!(is_newer_version("v0.2.0", "v0.1.1"));
        assert!(is_newer_version("v1.0.0", "v0.9.9"));
        assert!(!is_newer_version("v0.1.0", "v0.1.1"));
        assert!(!is_newer_version("v0.1.1", "0.1.1"));
        assert!(!is_newer_version("0.1.1", "v0.1.1"));
    }

    #[test]
    fn test_deserialize_github_release_json() {
        let json_data = r#"{
            "tag_name": "v0.1.1",
            "name": "zqlcrab v0.1.1",
            "html_url": "https://github.com/hyzwhu/zqlcrab/releases/tag/v0.1.1",
            "body": "Release notes here",
            "prerelease": false,
            "draft": false,
            "published_at": "2026-09-21T05:26:44Z"
        }"#;

        let rel: GithubRelease = serde_json::from_str(json_data).expect("Must parse");
        assert_eq!(rel.tag_name, "v0.1.1");
        assert_eq!(rel.name.as_deref(), Some("zqlcrab v0.1.1"));
        assert_eq!(
            rel.html_url,
            "https://github.com/hyzwhu/zqlcrab/releases/tag/v0.1.1"
        );
        assert!(!rel.prerelease);
        assert!(!rel.draft);
    }
}
