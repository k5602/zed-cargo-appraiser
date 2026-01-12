//! Zed extension for cargo-appraiser LSP
//!
//! This extension manages the cargo-appraiser language server, which provides
//! quality-of-life improvements for Cargo.toml files including:
//! - Version decorations and hover information
//! - Code actions for dependency updates
//! - Audit/vulnerability warnings
//!
//! See: https://github.com/washanhanzi/cargo-appraiser

use std::fs;
use zed::LanguageServerId;
use zed_extension_api::{self as zed, settings::LspSettings, Result};

/// The GitHub repository for cargo-appraiser releases
const GITHUB_REPO: &str = "washanhanzi/cargo-appraiser";

/// Main extension struct
struct CargoAppraiser {
    /// Cached binary path for the current session
    cached_binary_path: Option<String>,
}

impl CargoAppraiser {
    /// Get the asset name for the current platform
    fn get_asset_name() -> String {
        let (platform, arch) = zed::current_platform();

        format!(
            "cargo-appraiser-{os}-{arch}{ext}",
            arch = match arch {
                zed::Architecture::Aarch64 => "arm64",
                zed::Architecture::X86 | zed::Architecture::X8664 => "amd64",
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
        )
    }

    /// Get the binary name for the current platform
    fn get_binary_name() -> &'static str {
        let (platform, _) = zed::current_platform();
        match platform {
            zed::Os::Windows => "cargo-appraiser.exe",
            zed::Os::Mac | zed::Os::Linux => "cargo-appraiser",
        }
    }

    /// Fetch the latest release using Zed's built-in GitHub API
    fn fetch_latest_release(&self) -> Result<zed::GithubRelease> {
        zed::latest_github_release(
            GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )
    }

    /// Fetch a specific release by tag name
    fn fetch_release_by_tag(&self, tag: &str) -> Result<zed::GithubRelease> {
        zed::github_release_by_tag_name(GITHUB_REPO, tag)
    }

    /// Find a release matching a semver version requirement
    fn find_compatible_release(&self, version_req_str: &str) -> Result<zed::GithubRelease> {
        // First try to parse as an exact version (e.g., "=0.3.0" or "0.3.0")
        let trimmed = version_req_str.trim_start_matches('=');

        // Try fetching by exact tag first (faster if it exists)
        if let Ok(release) = self.fetch_release_by_tag(&format!("v{}", trimmed)) {
            return Ok(release);
        }
        if let Ok(release) = self.fetch_release_by_tag(trimmed) {
            return Ok(release);
        }

        // If that doesn't work, fall back to latest and validate
        let version_req = semver::VersionReq::parse(version_req_str)
            .map_err(|e| format!("Invalid version requirement '{}': {}", version_req_str, e))?;

        let release = self.fetch_latest_release()?;
        let release_version = semver::Version::parse(&release.version).map_err(|e| {
            format!(
                "Failed to parse release version '{}': {}",
                release.version, e
            )
        })?;

        if version_req.matches(&release_version) {
            Ok(release)
        } else {
            Err(format!(
                "Latest version {} does not match requirement '{}'. \
                 Consider updating the version requirement or removing it to use latest.",
                release.version, version_req_str
            ))
        }
    }

    /// Clean up old version directories, keeping only the specified one
    fn cleanup_old_versions(&self, keep_version_dir: &str) {
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Some(name_str) = name.to_str() {
                    // Only remove cargo-appraiser-* directories that aren't the current version
                    if name_str.starts_with("cargo-appraiser-") && name_str != keep_version_dir {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    }

    /// Download and install the language server binary
    fn install_binary(
        &mut self,
        language_server_id: &LanguageServerId,
        version: Option<&str>,
    ) -> Result<String> {
        // Set status to checking for updates
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        // Fetch the appropriate release
        let release = match version {
            Some(v) => self.find_compatible_release(v)?,
            None => self.fetch_latest_release()?,
        };

        // Find the asset for our platform
        let asset_name = Self::get_asset_name();
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "No binary available for your platform (looking for '{}'). \n\
                     Available assets: {:?}",
                    asset_name,
                    release.assets.iter().map(|a| &a.name).collect::<Vec<_>>()
                )
            })?;

        // Set up version directory
        let version_dir = format!("cargo-appraiser-{}", release.version);
        fs::create_dir_all(&version_dir)
            .map_err(|e| format!("Failed to create directory '{}': {}", version_dir, e))?;

        let binary_path = format!("{}/{}", version_dir, Self::get_binary_name());

        // Download if not already present
        if !fs::metadata(&binary_path).map_or(false, |m| m.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &binary_path,
                zed::DownloadedFileType::Uncompressed,
            )
            .map_err(|e| {
                format!(
                    "Failed to download cargo-appraiser v{}: {}. \n\
                     Please check your network connection and try again.",
                    release.version, e
                )
            })?;

            zed::make_file_executable(&binary_path)
                .map_err(|e| format!("Failed to make binary executable: {}", e))?;

            // Clean up old versions after successful download
            self.cleanup_old_versions(&version_dir);
        }

        // Cache the binary path
        self.cached_binary_path = Some(binary_path.clone());

        Ok(binary_path)
    }

    /// Get the binary path, using cached value if available
    fn get_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        version: Option<&str>,
    ) -> Result<String> {
        // If we have a cached path that still exists, use it
        if let Some(ref path) = self.cached_binary_path {
            if fs::metadata(path).map_or(false, |m| m.is_file()) {
                return Ok(path.clone());
            }
        }

        // Otherwise, install/download the binary
        self.install_binary(language_server_id, version)
    }
}

impl zed::Extension for CargoAppraiser {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree("cargo-appraiser", worktree)?;

        // Determine the binary path
        let path = if let Some(binary_path) = settings.binary.as_ref().and_then(|b| b.path.as_ref())
        {
            // User specified a custom binary path
            binary_path.clone()
        } else {
            // Get version from settings, if specified
            let version = settings
                .settings
                .as_ref()
                .and_then(|s| s.get("version"))
                .and_then(|v| v.as_str());

            self.get_binary_path(language_server_id, version)?
        };

        // Build command arguments
        // Default to inlayHint renderer which is recommended for Zed
        let mut args = vec!["--renderer".to_string(), "inlayHint".to_string()];

        // Check for custom arguments from binary settings
        if let Some(custom_args) = settings.binary.as_ref().and_then(|b| b.arguments.as_ref()) {
            args = custom_args.clone();
        }

        Ok(zed::Command {
            command: path,
            args,
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree("cargo-appraiser", worktree)?;
        Ok(settings.initialization_options)
    }
}

zed::register_extension!(CargoAppraiser);
