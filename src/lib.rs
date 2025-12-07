use std::fs;
use zed::http_client::{HttpMethod, HttpRequest};
use zed::LanguageServerId;
use zed_extension_api::{self as zed, settings::LspSettings, Result};

struct CargoAppraiser {}

#[derive(Debug, serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

impl CargoAppraiser {
    /// Parse a release tag into a semver Version.
    fn parse_release_version(tag: &str) -> Option<semver::Version> {
        let version_str = tag.trim_start_matches('v');
        semver::Version::parse(version_str).ok()
    }

    fn fetch_releases(&self) -> Result<Vec<GithubRelease>> {
        let request = HttpRequest::builder()
            .method(HttpMethod::Get)
            .url("https://api.github.com/repos/washanhanzi/cargo-appraiser/releases?per_page=100")
            .header("User-Agent", "zed-cargo-appraiser")
            .build()?;

        let response = zed::http_client::fetch(&request)
            .map_err(|e| format!("failed to fetch releases: {e}"))?;

        serde_json::from_slice(&response.body)
            .map_err(|e| format!("failed to parse GitHub releases: {e}"))
    }

    fn find_latest_release(&self) -> Result<GithubRelease> {
        let releases = self.fetch_releases()?;

        // Find the first (latest) non-prerelease with assets
        releases
            .into_iter()
            .find(|r| !r.prerelease && !r.assets.is_empty())
            .ok_or_else(|| "no stable release found".to_string())
    }

    fn find_compatible_release(&self, version_req_str: &str) -> Result<GithubRelease> {
        let version_req = semver::VersionReq::parse(version_req_str)
            .map_err(|e| format!("invalid version requirement '{version_req_str}': {e}"))?;
        let releases = self.fetch_releases()?;

        // Find the first (latest) release that matches the version requirement and has assets
        releases
            .into_iter()
            .filter(|r| !r.prerelease && !r.assets.is_empty())
            .find(|r| {
                Self::parse_release_version(&r.tag_name).is_some_and(|v| version_req.matches(&v))
            })
            .ok_or_else(|| format!("no release found matching '{version_req_str}'"))
    }

    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        version: Option<&str>,
    ) -> Result<String> {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = match version {
            Some(v) => self.find_compatible_release(v)?,
            None => self.find_latest_release()?,
        };

        let (platform, arch) = zed::current_platform();

        let asset_name = format!(
            "cargo-appraiser-{os}-{arch}{ext}",
            arch = match arch {
                zed::Architecture::Aarch64 => "arm64",
                zed::Architecture::X86 => "amd64",
                zed::Architecture::X8664 => "amd64",
            },
            os = match platform {
                zed::Os::Mac => "darwin",
                zed::Os::Linux => "linux",
                zed::Os::Windows => "windows",
            },
            ext = if platform == zed::Os::Windows {
                ".exe"
            } else {
                ""
            },
        );

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let version_tag = release.tag_name.trim_start_matches('v');
        let version_dir = format!("cargo-appraiser-{}", version_tag);
        fs::create_dir_all(&version_dir)
            .map_err(|err| format!("failed to create directory '{version_dir}': {err}"))?;

        let binary_path = format!(
            "{version_dir}/{bin_name}",
            bin_name = match platform {
                zed::Os::Windows => "cargo-appraiser.exe",
                zed::Os::Mac | zed::Os::Linux => "cargo-appraiser",
            }
        );

        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.browser_download_url,
                &binary_path,
                zed::DownloadedFileType::Uncompressed,
            )
            .map_err(|err| format!("failed to download file: {err}"))?;

            zed::make_file_executable(&binary_path)?;

            let entries = fs::read_dir(".")
                .map_err(|err| format!("failed to list working directory {err}"))?;
            for entry in entries {
                let entry = entry.map_err(|err| format!("failed to load directory entry {err}"))?;
                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        Ok(binary_path)
    }
}

impl zed::Extension for CargoAppraiser {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree("cargo-appraiser", worktree)?;

        // If user specified a binary path, use it directly
        let path = if let Some(binary_path) = settings.binary.as_ref().and_then(|b| b.path.as_ref())
        {
            binary_path.clone()
        } else {
            // Check for version in settings, fetch latest if not specified
            let version = settings
                .settings
                .as_ref()
                .and_then(|s| s.get("version"))
                .and_then(|v| v.as_str());

            self.language_server_binary_path(language_server_id, version)?
        };

        Ok(zed::Command {
            command: path,
            args: vec!["--renderer".to_string(), "inlayHint".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(CargoAppraiser);
