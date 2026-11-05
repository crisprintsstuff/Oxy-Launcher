use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app_info::AppId;
use crate::version::Version;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetectionSource {
    Manifest,
    WellKnownLocation,
    Registry,
    PathEnv,
    SiblingDir,
    Manual,
}

impl std::fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectionSource::Manifest => write!(f, "OxyStudio Managed"),
            DetectionSource::WellKnownLocation => write!(f, "Standard System Location"),
            DetectionSource::Registry => write!(f, "Windows Registry"),
            DetectionSource::PathEnv => write!(f, "System PATH"),
            DetectionSource::SiblingDir => write!(f, "Local Build / Sibling Repo"),
            DetectionSource::Manual => write!(f, "Manually Configured"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DetectedApp {
    pub app_id: AppId,
    pub executable_path: PathBuf,
    pub install_dir: PathBuf,
    pub version: Option<Version>,
    pub source: DetectionSource,
}

pub struct AppDetector;

impl AppDetector {
    /// Scan the system for an installed copy of the given app.
    pub fn detect(app_id: AppId) -> Option<DetectedApp> {
        // 1. Check registry on Windows
        #[cfg(target_os = "windows")]
        if let Some(detected) = Self::detect_from_windows_registry(app_id) {
            return Some(detected);
        }

        // 2. Check well-known install directories
        if let Some(detected) = Self::detect_from_well_known_locations(app_id) {
            return Some(detected);
        }

        // 3. Check system PATH
        if let Some(detected) = Self::detect_from_path(app_id) {
            return Some(detected);
        }

        // 4. Check sibling / development directories
        if let Some(detected) = Self::detect_from_sibling_dirs(app_id) {
            return Some(detected);
        }

        None
    }

    /// Extract version by invoking `<binary> --version`
    pub fn probe_version(exe_path: &Path) -> Option<Version> {
        let mut cmd = Command::new(exe_path);
        cmd.arg("--version");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let output = cmd.output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        // Look for version patterns like "oxypixel 1.0.0" or "oxybrush 0.4.0-alpha" or "v1.0.0"
        for word in text.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
            if let Ok(ver) = Version::parse(clean) {
                return Some(ver);
            }
        }

        None
    }

    fn detect_from_well_known_locations(app_id: AppId) -> Option<DetectedApp> {
        let candidates = Self::well_known_candidates(app_id);
        for exe in candidates {
            if exe.is_file() {
                let install_dir = exe.parent().unwrap_or(&exe).to_path_buf();
                let version = Self::probe_version(&exe);
                return Some(DetectedApp {
                    app_id,
                    executable_path: exe,
                    install_dir,
                    version,
                    source: DetectionSource::WellKnownLocation,
                });
            }
        }
        None
    }

    fn well_known_candidates(app_id: AppId) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let binary_name = app_id.binary_name();

        #[cfg(target_os = "windows")]
        {
            if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
                let base = PathBuf::from(&local_appdata);
                // OxyPixel installer puts in %LOCALAPPDATA%\OxyPixel\oxypixel.exe
                paths.push(base.join("OxyPixel").join(binary_name));
                // Inno Setup puts in %LOCALAPPDATA%\Programs\<App>\...
                paths.push(
                    base.join("Programs")
                        .join(app_id.display_name())
                        .join(binary_name),
                );
                paths.push(base.join(app_id.display_name()).join(binary_name));
            }

            if let Ok(prog_files) = env::var("ProgramFiles") {
                let base = PathBuf::from(prog_files);
                paths.push(base.join(app_id.display_name()).join(binary_name));
            }

            if let Ok(prog_files_x86) = env::var("ProgramFiles(x86)") {
                let base = PathBuf::from(prog_files_x86);
                paths.push(base.join(app_id.display_name()).join(binary_name));
            }

            if let Ok(appdata) = env::var("APPDATA") {
                let base = PathBuf::from(appdata);
                paths.push(base.join(app_id.display_name()).join(binary_name));
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            paths.push(PathBuf::from(format!("/usr/bin/{binary_name}")));
            paths.push(PathBuf::from(format!("/usr/local/bin/{binary_name}")));
            if let Some(home) = dirs::home_dir() {
                paths.push(home.join(".local").join("bin").join(binary_name));
            }
        }

        #[cfg(target_os = "macos")]
        {
            let app_bundle_name = format!("{}.app", app_id.display_name());
            paths.push(
                PathBuf::from("/Applications")
                    .join(&app_bundle_name)
                    .join("Contents/MacOS")
                    .join(binary_name),
            );
            if let Some(home) = dirs::home_dir() {
                paths.push(
                    home.join("Applications")
                        .join(&app_bundle_name)
                        .join("Contents/MacOS")
                        .join(binary_name),
                );
            }
        }

        paths
    }

    fn detect_from_path(app_id: AppId) -> Option<DetectedApp> {
        let binary_name = app_id.binary_name();
        let path_var = env::var_os("PATH")?;

        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if candidate.is_file() {
                let install_dir = candidate.parent().unwrap_or(&candidate).to_path_buf();
                let version = Self::probe_version(&candidate);
                return Some(DetectedApp {
                    app_id,
                    executable_path: candidate,
                    install_dir,
                    version,
                    source: DetectionSource::PathEnv,
                });
            }
        }

        None
    }

    fn detect_from_sibling_dirs(app_id: AppId) -> Option<DetectedApp> {
        let current_dir = env::current_dir().ok()?;
        let parent_dir = current_dir.parent()?;
        let binary_name = app_id.binary_name();

        let folder_name = match app_id {
            AppId::OxyBrush => "OxyBrush",
            AppId::OxyPixel => "oxypixel",
        };

        let sibling_root = parent_dir.join(folder_name);
        if !sibling_root.exists() {
            return None;
        }

        let candidates = [
            sibling_root.join("dist").join(binary_name),
            sibling_root
                .join("target")
                .join("release")
                .join(binary_name),
            sibling_root
                .join("target")
                .join("x86_64-pc-windows-msvc")
                .join("release")
                .join(binary_name),
            sibling_root
                .join("target")
                .join("x86_64-unknown-linux-gnu")
                .join("release")
                .join(binary_name),
            sibling_root.join(binary_name),
        ];

        for candidate in candidates {
            if candidate.is_file() {
                let install_dir = candidate.parent().unwrap_or(&candidate).to_path_buf();
                let version = Self::probe_version(&candidate);
                return Some(DetectedApp {
                    app_id,
                    executable_path: candidate,
                    install_dir,
                    version,
                    source: DetectionSource::SiblingDir,
                });
            }
        }

        None
    }

    #[cfg(target_os = "windows")]
    fn detect_from_windows_registry(app_id: AppId) -> Option<DetectedApp> {
        // Query keys using reg.exe which is universally present on Windows
        let reg_subkeys = match app_id {
            AppId::OxyPixel => {
                vec![r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyPixel"]
            }
            AppId::OxyBrush => vec![
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{66DE57B5-04C1-4FDB-93CD-ED8EAEA50F14}_is1",
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyBrush",
            ],
        };

        let hives = ["HKCU", "HKLM"];

        for hive in hives {
            for subkey in &reg_subkeys {
                let full_key = format!(r"{hive}\{subkey}");

                // 1. Try reading InstallLocation
                if let Some(dir_str) = Self::query_registry_value(&full_key, "InstallLocation") {
                    let dir = PathBuf::from(dir_str);
                    let candidate = dir.join(app_id.binary_name());
                    if candidate.is_file() {
                        let version = Self::query_registry_value(&full_key, "DisplayVersion")
                            .and_then(|v| Version::parse(&v).ok())
                            .or_else(|| Self::probe_version(&candidate));

                        return Some(DetectedApp {
                            app_id,
                            executable_path: candidate,
                            install_dir: dir,
                            version,
                            source: DetectionSource::Registry,
                        });
                    }
                }

                // 2. Try reading DisplayIcon (which in Inno Setup points to the exe directly)
                if let Some(icon_str) = Self::query_registry_value(&full_key, "DisplayIcon") {
                    let icon_clean = icon_str.split(',').next().unwrap_or(&icon_str).trim();
                    let candidate = PathBuf::from(icon_clean);
                    if candidate.is_file()
                        && candidate.file_name().is_some_and(|n| {
                            n.to_string_lossy()
                                .eq_ignore_ascii_case(app_id.binary_name())
                        })
                    {
                        let dir = candidate.parent().unwrap_or(&candidate).to_path_buf();
                        let version = Self::query_registry_value(&full_key, "DisplayVersion")
                            .and_then(|v| Version::parse(&v).ok())
                            .or_else(|| Self::probe_version(&candidate));

                        return Some(DetectedApp {
                            app_id,
                            executable_path: candidate,
                            install_dir: dir,
                            version,
                            source: DetectionSource::Registry,
                        });
                    }
                }
            }
        }

        None
    }

    #[cfg(target_os = "windows")]
    fn query_registry_value(key: &str, value_name: &str) -> Option<String> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mut cmd = Command::new("reg");
        cmd.args(["query", key, "/v", value_name]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.starts_with(value_name) {
                // Format: <ValueName>    REG_SZ    <Data>
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Rejoin remainder in case path has spaces
                    let data = parts[2..].join(" ");
                    return Some(data.trim().to_string());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_non_empty() {
        let candidates = AppDetector::well_known_candidates(AppId::OxyBrush);
        assert!(!candidates.is_empty());
    }
}
