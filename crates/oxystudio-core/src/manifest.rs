use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app_info::AppId;
use crate::error::Result;
use crate::version::Version;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledApp {
    pub app_id: AppId,
    pub version: Version,
    pub tag: String,
    pub executable_path: PathBuf,
    pub install_dir: PathBuf,
    pub installed_at: u64,
}

impl InstalledApp {
    pub fn is_valid(&self) -> bool {
        self.executable_path.is_file()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub apps: HashMap<AppId, InstalledApp>,
}

impl Manifest {
    pub fn load_or_default(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn get(&self, app_id: AppId) -> Option<&InstalledApp> {
        self.apps.get(&app_id)
    }

    pub fn register(
        &mut self,
        app_id: AppId,
        version: Version,
        tag: String,
        executable_path: PathBuf,
        install_dir: PathBuf,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.apps.insert(
            app_id,
            InstalledApp {
                app_id,
                version,
                tag,
                executable_path,
                install_dir,
                installed_at: now,
            },
        );
    }

    pub fn remove(&mut self, app_id: AppId) -> Option<InstalledApp> {
        self.apps.remove(&app_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_manifest_roundtrip() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("installed.json");

        let mut manifest = Manifest::default();
        manifest.register(
            AppId::OxyBrush,
            Version::parse("0.4.0-alpha").unwrap(),
            "v0.4.0-alpha".to_string(),
            dir.path().join("oxybrush"),
            dir.path().to_path_buf(),
        );

        manifest.save(&manifest_path).unwrap();

        let loaded = Manifest::load_or_default(&manifest_path);
        let app = loaded.get(AppId::OxyBrush).unwrap();
        assert_eq!(app.tag, "v0.4.0-alpha");
        assert_eq!(app.version.to_string(), "0.4.0-alpha");
    }
}
