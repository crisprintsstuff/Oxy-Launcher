use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppId {
    OxyBrush,
    OxyPixel,
}

impl AppId {
    pub const ALL: &'static [AppId] = &[AppId::OxyBrush, AppId::OxyPixel];

    pub fn as_str(&self) -> &'static str {
        match self {
            AppId::OxyBrush => "oxybrush",
            AppId::OxyPixel => "oxypixel",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AppId::OxyBrush => "OxyBrush",
            AppId::OxyPixel => "OxyPixel",
        }
    }

    pub fn repo(&self) -> &'static str {
        match self {
            AppId::OxyBrush => "crisprintsstuff/OxyBrush",
            AppId::OxyPixel => "crisprintsstuff/oxypixel",
        }
    }

    pub fn tagline(&self) -> &'static str {
        match self {
            AppId::OxyBrush => "Modern 3D digital sculpting & topology suite",
            AppId::OxyPixel => "Crisp pixel-art editor, tileset & animation studio",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AppId::OxyBrush => {
                "A high-performance voxel & dynamic mesh sculpting application built in Rust with wgpu. Features dynamic tessellation, multithreaded brush stamping, and comprehensive toolsets."
            }
            AppId::OxyPixel => {
                "A clean, dedicated pixel-art editor featuring multi-layer animations, tileset slicing, color palette indexing, and lightweight export workflows."
            }
        }
    }

    pub fn binary_stem(&self) -> &'static str {
        match self {
            AppId::OxyBrush => "oxybrush",
            AppId::OxyPixel => "oxypixel",
        }
    }

    pub fn binary_name(&self) -> &'static str {
        if cfg!(windows) {
            match self {
                AppId::OxyBrush => "oxybrush.exe",
                AppId::OxyPixel => "oxypixel.exe",
            }
        } else {
            match self {
                AppId::OxyBrush => "oxybrush",
                AppId::OxyPixel => "oxypixel",
            }
        }
    }

    /// Match a release asset filename for the current OS and architecture.
    pub fn matches_current_platform(&self, asset_name: &str) -> bool {
        let name_lower = asset_name.to_lowercase();
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        // Skip checksums, signatures, source code archives
        if name_lower.ends_with(".txt")
            || name_lower.ends_with(".sha256")
            || name_lower.ends_with(".sig")
            || name_lower.starts_with("source")
        {
            return false;
        }

        match (os, arch) {
            ("linux", "x86_64") => match self {
                AppId::OxyBrush => name_lower.contains("x86_64") && name_lower.ends_with(".tar.gz"),
                AppId::OxyPixel => {
                    name_lower.contains("x86_64")
                        && (name_lower.ends_with(".tar.gz") || name_lower.ends_with(".appimage"))
                }
            },
            ("windows", "x86_64") => match self {
                AppId::OxyBrush => {
                    name_lower.contains("windows")
                        && name_lower.contains("x86_64")
                        && (name_lower.ends_with(".zip") || name_lower.ends_with(".exe"))
                }
                AppId::OxyPixel => {
                    name_lower.contains("windows")
                        && name_lower.contains("x86_64")
                        && (name_lower.ends_with(".exe") || name_lower.ends_with(".zip"))
                }
            },
            ("macos", _) => name_lower.contains("macos") && name_lower.ends_with(".dmg"),
            _ => false,
        }
    }
}

impl std::str::FromStr for AppId {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oxybrush" | "oxy-brush" => Ok(AppId::OxyBrush),
            "oxypixel" | "oxy-pixel" => Ok(AppId::OxyPixel),
            other => Err(crate::error::Error::NoReleaseFound(format!(
                "unknown app: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_real_release_assets() {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert!(AppId::OxyBrush
                .matches_current_platform("OxyBrush-0.4.0-alpha-linux-x86_64.tar.gz"));
            assert!(!AppId::OxyBrush
                .matches_current_platform("OxyBrush-0.4.0-alpha-linux-x86_64.flatpak"));
            assert!(!AppId::OxyBrush.matches_current_platform("SHA256SUMS.txt"));

            assert!(AppId::OxyPixel.matches_current_platform("oxypixel-1.0.0-linux-x86_64.tar.gz"));
            assert!(AppId::OxyPixel.matches_current_platform("OxyPixel-1.0.0-x86_64.AppImage"));
            assert!(!AppId::OxyPixel.matches_current_platform("oxypixel-1.0.0-linux-x86_64.deb"));
            assert!(!AppId::OxyPixel.matches_current_platform("SHA256SUMS.txt"));
        }
    }
}
