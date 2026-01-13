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
    /// Cached binary path and version for the current session
    cached_binary: Option<CachedBinary>,
}

/// Cached binary information
struct CachedBinary {
    path: String,
    version: Option<String>,
}

impl CargoAppraiser {
    /// Get the asset name for the current platform
    fn get_asset_name() -> String {
        let (platform, arch) = zed::current_platform();

        format!(
            "cargo-appraiser-{os}-{arch}{ext}",
            arch = match arch {
                zed::Architecture::Aarch64 => "arm64",
                // Note: 32-bit x86 binaries are not provided, but we map to amd64
                // and let it fail gracefully when the binary is not found
                zed::Architecture::X86 => "x86",
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

    /// Fetch a specific release by exact version string
    ///
    /// Only exact versions are supported (e.g., "0.3.0" or "=0.3.0").
    /// Semver ranges like "^0.3.0" or ">=0.3.0" are not supported because
    /// Zed's extension API does not provide paginated access to GitHub releases.
    fn fetch_release_by_version(&self, version: &str) -> Result<zed::GithubRelease> {
        // Check for semver range operators which we don't support
        let has_range_operator = version.starts_with('^')
            || version.starts_with('~')
            || version.starts_with('>')
            || version.starts_with('<')
            || version.contains(',')
            || version.contains(' ');

        if has_range_operator {
            return Err(format!(
                "Semver ranges like '{}' are not supported.\n\
                Please specify an exact version (e.g., \"0.3.0\") or \
                remove the version setting to use the latest release.",
                version
            ));
        }

        // Normalize: strip leading 'v' or '=' if present, user can write "1.0.5", "v1.0.5", or "=1.0.5"
        let version = version
            .trim()
            .trim_start_matches('v')
            .trim_start_matches('=');

        // Upstream uses consistent v-prefixed tags (e.g., v1.0.5, v0.3.2)
        let tag = format!("v{}", version);

        zed::github_release_by_tag_name(GITHUB_REPO, &tag).map_err(|_| {
            format!(
                "Version '{}' not found.\n\
                Please check available releases at:\n\
                https://github.com/{}/releases",
                version, GITHUB_REPO
            )
        })
    }

    /// Clean up old version directories, keeping only the specified one
    fn cleanup_old_versions(&self, keep_version_dir: &str) {
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Some(name_str) = name.to_str() {
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
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        // Fetch the appropriate release
        let release = match version {
            Some(v) => self.fetch_release_by_version(v)?,
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
                    "No binary available for your platform (looking for '{}').\n\
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
                    "Failed to download cargo-appraiser v{}: {}.\n\
                    Please check your network connection and try again.",
                    release.version, e
                )
            })?;

            zed::make_file_executable(&binary_path)
                .map_err(|e| format!("Failed to make binary executable: {}", e))?;

            // Clean up old versions after successful download
            self.cleanup_old_versions(&version_dir);
        }

        // Cache the binary path and version
        self.cached_binary = Some(CachedBinary {
            path: binary_path.clone(),
            version: version.map(|s| s.to_string()),
        });

        Ok(binary_path)
    }

    /// Get the binary path, using cached value if available and version matches
    fn get_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        version: Option<&str>,
    ) -> Result<String> {
        // Check if we have a valid cached path for the requested version
        if let Some(ref cached) = self.cached_binary {
            let version_matches = match (&cached.version, version) {
                (None, None) => true,
                (Some(cached_v), Some(requested_v)) => cached_v == requested_v,
                _ => false,
            };

            if version_matches && fs::metadata(&cached.path).map_or(false, |m| m.is_file()) {
                return Ok(cached.path.clone());
            }
        }

        self.install_binary(language_server_id, version)
    }
}

impl zed::Extension for CargoAppraiser {
    fn new() -> Self {
        Self {
            cached_binary: None,
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
            binary_path.clone()
        } else {
            let version = settings
                .settings
                .as_ref()
                .and_then(|s| s.get("version"))
                .and_then(|v| v.as_str());

            self.get_binary_path(language_server_id, version)?
        };

        // Build command arguments - default to inlayHint renderer for Zed
        let mut args = vec!["--renderer".to_string(), "inlayHint".to_string()];

        // Check for custom arguments from binary settings
        if let Some(custom_args) = settings.binary.as_ref().and_then(|b| b.arguments.as_ref()) {
            // If user specifies --renderer, use their args as full replacement
            // Otherwise, append their args to defaults
            if custom_args.iter().any(|arg| arg == "--renderer") {
                args = custom_args.clone();
            } else {
                args.extend(custom_args.iter().cloned());
            }
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
