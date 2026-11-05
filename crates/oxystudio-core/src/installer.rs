use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

use crate::app_info::AppId;
use crate::checksum::{parse_checksums, verify_bytes};
use crate::error::{Error, Result};
use crate::github::{find_checksum_asset, select_app_asset, Asset, GitHubClient, Release};
use crate::manifest::Manifest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallStage {
    Preparing,
    DownloadingChecksums,
    DownloadingAsset,
    VerifyingChecksum,
    Extracting,
    ConfiguringPermissions,
    Completed,
}

#[derive(Clone, Debug)]
pub struct InstallProgress {
    pub stage: InstallStage,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

pub type ProgressCallback = Box<dyn Fn(InstallProgress) + Send + Sync>;

pub struct Installer {
    apps_root: PathBuf,
    manifest_path: PathBuf,
    client: GitHubClient,
}

impl Installer {
    pub fn new(apps_root: PathBuf, manifest_path: PathBuf) -> Self {
        Self {
            apps_root,
            manifest_path,
            client: GitHubClient::new(),
        }
    }

    /// Standard default paths for oxystudio
    pub fn default_paths() -> (PathBuf, PathBuf) {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oxystudio");
        let apps_dir = base.join("apps");
        let manifest = base.join("installed.json");
        (apps_dir, manifest)
    }

    pub fn default_installer() -> Self {
        let (apps_root, manifest_path) = Self::default_paths();
        Self::new(apps_root, manifest_path)
    }

    pub fn load_manifest(&self) -> Manifest {
        Manifest::load_or_default(&self.manifest_path)
    }

    pub fn save_manifest(&self, manifest: &Manifest) -> Result<()> {
        manifest.save(&self.manifest_path)
    }

    /// Install or update an application from a specific release
    pub fn install_release(
        &self,
        app: AppId,
        release: &Release,
        on_progress: Option<&ProgressCallback>,
    ) -> Result<PathBuf> {
        let report = |stage, bytes, total, msg: &str| {
            if let Some(cb) = on_progress {
                cb(InstallProgress {
                    stage,
                    bytes_downloaded: bytes,
                    total_bytes: total,
                    message: msg.to_string(),
                });
            }
        };

        report(
            InstallStage::Preparing,
            0,
            None,
            "Preparing installation...",
        );

        // 1. Find the target binary asset for our platform
        let asset = select_app_asset(release, app).ok_or_else(|| Error::NoCompatibleAsset {
            app: app.display_name().to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })?;

        // 2. Fetch and parse checksums if available
        let mut expected_checksum: Option<String> = None;
        if let Some(checksum_asset) = find_checksum_asset(release) {
            report(
                InstallStage::DownloadingChecksums,
                0,
                None,
                "Fetching release checksums...",
            );
            if let Ok(checksum_text) = self.client.fetch_text(&checksum_asset.browser_download_url)
            {
                let checksums = parse_checksums(&checksum_text);
                expected_checksum = checksums.get(&asset.name).cloned();
            }
        }

        // 3. Download the asset into memory or temp buffer
        report(
            InstallStage::DownloadingAsset,
            0,
            Some(asset.size),
            &format!("Downloading {}...", asset.name),
        );

        let data = self.download_with_progress(asset, on_progress)?;

        // 4. Verify checksum
        if let Some(expected) = &expected_checksum {
            report(
                InstallStage::VerifyingChecksum,
                data.len() as u64,
                Some(asset.size),
                "Verifying SHA-256 checksum...",
            );
            verify_bytes(&data, &asset.name, expected)?;
        }

        // 5. Extract to staging directory
        report(
            InstallStage::Extracting,
            data.len() as u64,
            Some(asset.size),
            "Extracting application files...",
        );

        let app_dir = self.apps_root.join(app.as_str());
        let staging_dir = self.apps_root.join(format!(".staging-{}", app.as_str()));

        if staging_dir.exists() {
            let _ = fs::remove_dir_all(&staging_dir);
        }
        fs::create_dir_all(&staging_dir)?;

        Self::extract_asset_data(&asset.name, &data, &staging_dir)?;

        // 6. Find target executable within staging
        let exe_path = Self::find_executable(&staging_dir, app.binary_name())
            .ok_or_else(|| Error::ExecutableNotFound(staging_dir.clone()))?;

        // Relative path from staging_dir to the executable
        let rel_exe = exe_path
            .strip_prefix(&staging_dir)
            .map_err(|_| Error::ExecutableNotFound(exe_path.clone()))?
            .to_path_buf();

        // 7. Configure executable permissions on Unix
        report(
            InstallStage::ConfiguringPermissions,
            data.len() as u64,
            Some(asset.size),
            "Setting executable permissions...",
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&exe_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&exe_path, perms);
            }
        }

        // 8. Swap staging into place atomically
        if app_dir.exists() {
            let backup_dir = self.apps_root.join(format!(".backup-{}", app.as_str()));
            if backup_dir.exists() {
                let _ = fs::remove_dir_all(&backup_dir);
            }
            fs::rename(&app_dir, &backup_dir)?;
            if let Err(err) = fs::rename(&staging_dir, &app_dir) {
                // Try restore backup
                let _ = fs::rename(&backup_dir, &app_dir);
                return Err(Error::Io(err));
            }
            let _ = fs::remove_dir_all(&backup_dir);
        } else {
            fs::rename(&staging_dir, &app_dir)?;
        }

        let final_exe_path = app_dir.join(&rel_exe);

        // Ensure executable permissions on the final destination too
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&final_exe_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&final_exe_path, perms);
            }
        }

        // 9. Update manifest
        let mut manifest = self.load_manifest();
        let version = release.version()?;
        manifest.register(
            app,
            version,
            release.tag_name.clone(),
            final_exe_path.clone(),
            app_dir,
        );
        self.save_manifest(&manifest)?;

        report(
            InstallStage::Completed,
            data.len() as u64,
            Some(asset.size),
            "Installation completed successfully!",
        );

        Ok(final_exe_path)
    }

    /// Uninstall an application
    pub fn uninstall(&self, app: AppId) -> Result<()> {
        let app_dir = self.apps_root.join(app.as_str());
        if app_dir.exists() {
            fs::remove_dir_all(&app_dir)?;
        }
        let mut manifest = self.load_manifest();
        manifest.remove(app);
        self.save_manifest(&manifest)?;
        Ok(())
    }

    fn download_with_progress(
        &self,
        asset: &Asset,
        on_progress: Option<&ProgressCallback>,
    ) -> Result<Vec<u8>> {
        let resp = ureq::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .get(&asset.browser_download_url)
            .call()
            .map_err(|e| Error::Network(format!("failed to download asset {}: {e}", asset.name)))?;

        let total_size = resp
            .header("Content-Length")
            .and_then(|l| l.parse::<u64>().ok())
            .or(if asset.size > 0 {
                Some(asset.size)
            } else {
                None
            });

        let mut reader = resp.into_reader();
        let mut data = Vec::with_capacity(total_size.unwrap_or(4 * 1024 * 1024) as usize);
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..n]);
            downloaded += n as u64;

            if let Some(cb) = on_progress {
                cb(InstallProgress {
                    stage: InstallStage::DownloadingAsset,
                    bytes_downloaded: downloaded,
                    total_bytes: total_size,
                    message: format!(
                        "Downloading {} ({:.1} MB{})...",
                        asset.name,
                        downloaded as f64 / (1024.0 * 1024.0),
                        total_size.map_or(String::new(), |t| format!(
                            " / {:.1} MB",
                            t as f64 / (1024.0 * 1024.0)
                        ))
                    ),
                });
            }
        }

        Ok(data)
    }

    fn extract_asset_data(asset_name: &str, data: &[u8], destination: &Path) -> Result<()> {
        let name_lower = asset_name.to_lowercase();
        if name_lower.ends_with(".tar.gz") || name_lower.ends_with(".tgz") {
            let tar = GzDecoder::new(data);
            let mut archive = Archive::new(tar);
            archive.unpack(destination)?;
            Ok(())
        } else if name_lower.ends_with(".zip") {
            let cursor = io::Cursor::new(data);
            let mut zip = zip::ZipArchive::new(cursor)
                .map_err(|e| Error::Zip(format!("failed to open zip {asset_name}: {e}")))?;
            zip.extract(destination)
                .map_err(|e| Error::Zip(format!("failed to extract zip {asset_name}: {e}")))?;
            Ok(())
        } else {
            // Direct standalone binary (e.g. oxypixel-*.exe or AppImage)
            let target_path = destination.join(asset_name);
            fs::write(&target_path, data)?;
            Ok(())
        }
    }

    fn find_executable(dir: &Path, expected_name: &str) -> Option<PathBuf> {
        let direct = dir.join(expected_name);
        if direct.is_file() {
            return Some(direct);
        }

        // Search one or two levels deep
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        if file_name.eq_ignore_ascii_case(expected_name) {
                            return Some(path);
                        }
                    }
                } else if path.is_dir() {
                    let sub_direct = path.join(expected_name);
                    if sub_direct.is_file() {
                        return Some(sub_direct);
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tempfile::tempdir;

    #[test]
    fn extracts_tar_gz_and_finds_binary() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("extracted");
        fs::create_dir_all(&target).unwrap();

        // Build a small tar.gz in memory
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = tar::Builder::new(&mut enc);
            let mut header = tar::Header::new_gnu();
            let data = b"#!/bin/sh\necho hello\n";
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "oxybrush", &data[..]).unwrap();
            tar.finish().unwrap();
        }
        let gz_bytes = enc.finish().unwrap();

        Installer::extract_asset_data("oxybrush-linux.tar.gz", &gz_bytes, &target).unwrap();
        let found = Installer::find_executable(&target, "oxybrush");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), target.join("oxybrush"));
    }
}
