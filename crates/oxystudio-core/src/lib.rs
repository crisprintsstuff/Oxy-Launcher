//! Core library for OxyStudio launcher and package management.

pub mod app_info;
pub mod checksum;
pub mod error;
pub mod github;
pub mod installer;
pub mod launcher;
pub mod manifest;
pub mod version;

pub use app_info::AppId;
pub use error::{Error, Result};
pub use github::{Asset, GitHubClient, Release};
pub use installer::{InstallProgress, InstallStage, Installer, ProgressCallback};
pub use launcher::Launcher;
pub use manifest::{InstalledApp, Manifest};
pub use version::Version;
