use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use egui::{vec2, Align, Color32, CornerRadius, Layout, Margin, RichText};
use oxystudio_core::{
    AppId, GitHubClient, InstallProgress, Installer, Launcher, Manifest, Release,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherSettings {
    pub allow_prereleases: bool,
    pub auto_check_updates: bool,
    pub custom_apps_dir: Option<PathBuf>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            allow_prereleases: true,
            auto_check_updates: true,
            custom_apps_dir: None,
        }
    }
}

impl LauncherSettings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oxystudio")
            .join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub enum LauncherEvent {
    ReleasesFetched {
        app_id: AppId,
        releases: Vec<Release>,
    },
    FetchFailed {
        app_id: AppId,
        error: String,
    },
    Progress {
        app_id: AppId,
        progress: InstallProgress,
    },
    InstallCompleted {
        app_id: AppId,
        exe_path: PathBuf,
    },
    InstallFailed {
        app_id: AppId,
        error: String,
    },
    UninstallCompleted {
        app_id: AppId,
    },
    LaunchSuccess {
        app_id: AppId,
    },
    LaunchFailed {
        app_id: AppId,
        error: String,
    },
}

#[derive(Clone, Debug, Default)]
pub struct AppCardState {
    pub releases: Vec<Release>,
    pub is_fetching: bool,
    pub is_installing: bool,
    pub progress: Option<InstallProgress>,
    pub error: Option<String>,
    pub status_message: Option<String>,
}

pub struct OxyStudioApp {
    settings: LauncherSettings,
    settings_open: bool,
    selected_app: AppId,
    apps: HashMap<AppId, AppCardState>,
    manifest: Manifest,
    installer: Installer,
    tx: Sender<LauncherEvent>,
    rx: Receiver<LauncherEvent>,
    release_notes_modal: Option<(AppId, Release)>,
    global_status: Option<String>,
}

impl OxyStudioApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let settings = LauncherSettings::load();
        let (default_apps_root, default_manifest_path) = Installer::default_paths();
        let apps_root = settings
            .custom_apps_dir
            .clone()
            .unwrap_or(default_apps_root);

        let installer = Installer::new(apps_root, default_manifest_path);
        let manifest = installer.load_manifest();

        let (tx, rx) = channel();

        let mut apps = HashMap::new();
        for &app_id in AppId::ALL {
            apps.insert(app_id, AppCardState::default());
        }

        let mut app = Self {
            settings,
            settings_open: false,
            selected_app: AppId::OxyBrush,
            apps,
            manifest,
            installer,
            tx,
            rx,
            release_notes_modal: None,
            global_status: None,
        };

        // Trigger initial releases fetch
        app.refresh_all_releases();

        app
    }

    fn refresh_all_releases(&mut self) {
        self.global_status = Some("Checking GitHub for releases...".to_string());
        for &app_id in AppId::ALL {
            if let Some(card) = self.apps.get_mut(&app_id) {
                card.is_fetching = true;
                card.error = None;
            }

            let tx = self.tx.clone();
            let repo = app_id.repo().to_string();

            thread::spawn(move || {
                let client = GitHubClient::new();
                match client.fetch_releases(&repo) {
                    Ok(releases) => {
                        let _ = tx.send(LauncherEvent::ReleasesFetched { app_id, releases });
                    }
                    Err(e) => {
                        let _ = tx.send(LauncherEvent::FetchFailed {
                            app_id,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    fn start_install(&mut self, app_id: AppId, release: Release) {
        if let Some(card) = self.apps.get_mut(&app_id) {
            card.is_installing = true;
            card.error = None;
            card.progress = None;
            card.status_message = Some(format!("Preparing to install {}...", release.tag_name));
        }

        let tx = self.tx.clone();
        let installer = Installer::new(self.installer_apps_root(), self.installer_manifest_path());

        thread::spawn(move || {
            let tx_prog = tx.clone();
            let cb: oxystudio_core::ProgressCallback = Box::new(move |p| {
                let _ = tx_prog.send(LauncherEvent::Progress {
                    app_id,
                    progress: p,
                });
            });

            match installer.install_release(app_id, &release, Some(&cb)) {
                Ok(exe_path) => {
                    let _ = tx.send(LauncherEvent::InstallCompleted { app_id, exe_path });
                }
                Err(e) => {
                    let _ = tx.send(LauncherEvent::InstallFailed {
                        app_id,
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    fn start_uninstall(&mut self, app_id: AppId) {
        let installer = Installer::new(self.installer_apps_root(), self.installer_manifest_path());
        let tx = self.tx.clone();

        thread::spawn(move || {
            if let Err(e) = installer.uninstall(app_id) {
                let _ = tx.send(LauncherEvent::InstallFailed {
                    app_id,
                    error: e.to_string(),
                });
            } else {
                let _ = tx.send(LauncherEvent::UninstallCompleted { app_id });
            }
        });
    }

    fn launch_installed(&mut self, app_id: AppId) {
        if let Some(installed) = self.manifest.get(app_id) {
            match Launcher::launch(installed, &[]) {
                Ok(_) => {
                    let _ = self.tx.send(LauncherEvent::LaunchSuccess { app_id });
                }
                Err(e) => {
                    let _ = self.tx.send(LauncherEvent::LaunchFailed {
                        app_id,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    fn installer_apps_root(&self) -> PathBuf {
        let (default_apps, _) = Installer::default_paths();
        self.settings
            .custom_apps_dir
            .clone()
            .unwrap_or(default_apps)
    }

    fn installer_manifest_path(&self) -> PathBuf {
        let (_, default_manifest) = Installer::default_paths();
        default_manifest
    }

    fn poll_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                LauncherEvent::ReleasesFetched { app_id, releases } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.is_fetching = false;
                        card.releases = releases;
                    }
                    self.global_status = None;
                }
                LauncherEvent::FetchFailed { app_id, error } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.is_fetching = false;
                        card.error = Some(error);
                    }
                    self.global_status = None;
                }
                LauncherEvent::Progress { app_id, progress } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.status_message = Some(progress.message.clone());
                        card.progress = Some(progress);
                    }
                }
                LauncherEvent::InstallCompleted { app_id, exe_path } => {
                    self.manifest = self.installer.load_manifest();
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.is_installing = false;
                        card.progress = None;
                        card.status_message = Some(format!(
                            "Installed successfully ({})",
                            exe_path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }
                }
                LauncherEvent::InstallFailed { app_id, error } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.is_installing = false;
                        card.progress = None;
                        card.error = Some(error);
                    }
                }
                LauncherEvent::UninstallCompleted { app_id } => {
                    self.manifest = self.installer.load_manifest();
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.status_message = Some("Uninstalled successfully.".to_string());
                    }
                }
                LauncherEvent::LaunchSuccess { app_id } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.status_message = Some("Launched application.".to_string());
                    }
                }
                LauncherEvent::LaunchFailed { app_id, error } => {
                    if let Some(card) = self.apps.get_mut(&app_id) {
                        card.error = Some(error);
                    }
                }
            }
        }
    }

    fn open_directory(path: &Path) {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(path).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
    }
}

impl eframe::App for OxyStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // Any active downloads/installations require continuous repaint
        let any_installing = self.apps.values().any(|c| c.is_installing || c.is_fetching);
        if any_installing {
            ctx.request_repaint();
        }

        // Configure theme / style
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(20, 22, 28);
        visuals.window_fill = Color32::from_rgb(26, 29, 36);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 31, 40);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 40, 52);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 70);
        visuals.widgets.active.bg_fill = Color32::from_rgb(56, 64, 82);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
        visuals.widgets.active.corner_radius = CornerRadius::same(6);
        ctx.set_visuals(visuals);

        // Top Navigation Bar
        egui::TopBottomPanel::top("header_bar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(Color32::from_rgb(16, 18, 24))
                    .inner_margin(Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new("✦ Oxy Studio")
                            .color(Color32::from_rgb(56, 189, 248))
                            .size(20.0)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Creative Tools Hub")
                            .color(Color32::from_rgb(140, 150, 170))
                            .size(13.0),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("⚙ Settings").size(13.0)).clicked() {
                            self.settings_open = !self.settings_open;
                        }

                        let is_busy = self.apps.values().any(|c| c.is_fetching);
                        let refresh_btn = if is_busy {
                            ui.add_enabled(
                                false,
                                egui::Button::new(RichText::new("⟳ Checking...").size(13.0)),
                            )
                        } else {
                            ui.button(RichText::new("⟳ Refresh Releases").size(13.0))
                        };

                        if refresh_btn.clicked() {
                            self.refresh_all_releases();
                        }
                    });
                });
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(Color32::from_rgb(14, 16, 20))
                    .inner_margin(Margin::symmetric(16, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(status) = &self.global_status {
                        ui.label(
                            RichText::new(status)
                                .color(Color32::from_rgb(56, 189, 248))
                                .size(11.0),
                        );
                    } else {
                        ui.label(
                            RichText::new("Connected to GitHub API")
                                .color(Color32::from_rgb(90, 100, 120))
                                .size(11.0),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("v0.1.0")
                                .color(Color32::from_rgb(80, 90, 110))
                                .size(11.0),
                        );
                    });
                });
            });

        // Left App Selector Sidebar
        egui::SidePanel::left("apps_sidebar")
            .resizable(false)
            .exact_width(220.0)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(Color32::from_rgb(22, 25, 33))
                    .inner_margin(Margin::symmetric(12, 16)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("APPLICATIONS")
                        .size(11.0)
                        .color(Color32::from_rgb(110, 120, 140))
                        .strong(),
                );
                ui.add_space(8.0);

                for &app_id in AppId::ALL {
                    let is_selected = self.selected_app == app_id;
                    let installed = self.manifest.get(app_id).is_some();

                    let text_color = if is_selected {
                        Color32::from_rgb(255, 255, 255)
                    } else {
                        Color32::from_rgb(170, 180, 200)
                    };

                    let icon = match app_id {
                        AppId::OxyBrush => "🖌",
                        AppId::OxyPixel => "▦",
                    };

                    let label = format!("{} {}", icon, app_id.display_name());
                    let mut btn = egui::Button::new(
                        RichText::new(label).color(text_color).size(14.0).strong(),
                    )
                    .min_size(vec2(196.0, 36.0));

                    if is_selected {
                        btn = btn.fill(Color32::from_rgb(45, 55, 75));
                    }

                    if ui.add(btn).clicked() {
                        self.selected_app = app_id;
                    }

                    // Small indicator badge
                    ui.horizontal(|ui| {
                        ui.add_space(32.0);
                        if installed {
                            ui.label(
                                RichText::new("● Installed")
                                    .color(Color32::from_rgb(34, 197, 94))
                                    .size(10.0),
                            );
                        } else {
                            ui.label(
                                RichText::new("○ Available")
                                    .color(Color32::from_rgb(120, 130, 150))
                                    .size(10.0),
                            );
                        }
                    });

                    ui.add_space(4.0);
                }
            });

        // Central Panel: Selected App Detail Card
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .fill(Color32::from_rgb(26, 29, 38))
                    .inner_margin(Margin::same(24)),
            )
            .show(ctx, |ui| {
                self.show_app_detail(ui, self.selected_app);
            });

        // Settings Window
        if self.settings_open {
            let mut open = true;
            let mut close_clicked = false;
            egui::Window::new("Settings")
                .resizable(false)
                .collapsible(false)
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .min_width(420.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Launcher Configuration");
                    ui.add_space(12.0);

                    if ui
                        .checkbox(
                            &mut self.settings.allow_prereleases,
                            "Include Pre-releases (Alphas, Betas, Release Candidates)",
                        )
                        .changed()
                    {
                        self.settings.save();
                    }

                    if ui
                        .checkbox(
                            &mut self.settings.auto_check_updates,
                            "Check for updates automatically on start",
                        )
                        .changed()
                    {
                        self.settings.save();
                    }

                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Apps Installation Directory:")
                            .size(12.0)
                            .strong(),
                    );
                    let (default_apps, _) = Installer::default_paths();
                    let current_dir = self
                        .settings
                        .custom_apps_dir
                        .as_ref()
                        .unwrap_or(&default_apps);

                    ui.label(
                        RichText::new(current_dir.display().to_string())
                            .color(Color32::from_rgb(160, 170, 190))
                            .monospace()
                            .size(11.0),
                    );

                    ui.add_space(8.0);
                    if ui.button("📂 Open Apps Folder").clicked() {
                        Self::open_directory(current_dir);
                    }

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close_clicked = true;
                    }
                });
            if close_clicked {
                open = false;
            }
            self.settings_open = open;
        }

        // Release Notes Modal
        if let Some((app_id, release)) = self.release_notes_modal.clone() {
            let mut open = true;
            egui::Window::new(format!(
                "Release Notes: {} {}",
                app_id.display_name(),
                release.tag_name
            ))
            .open(&mut open)
            .min_width(520.0)
            .max_height(480.0)
            .show(ctx, |ui| {
                ui.heading(release.display_title());
                if let Some(published) = &release.published_at {
                    ui.label(
                        RichText::new(format!("Published: {}", published))
                            .size(11.0)
                            .color(Color32::from_rgb(140, 150, 170)),
                    );
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(body) = &release.body {
                        ui.label(body);
                    } else {
                        ui.label("No release notes provided for this version.");
                    }
                });
            });

            if !open {
                self.release_notes_modal = None;
            }
        }
    }
}

impl OxyStudioApp {
    fn show_app_detail(&mut self, ui: &mut egui::Ui, app_id: AppId) {
        let card = self.apps.get(&app_id).cloned().unwrap_or_default();
        let installed = self.manifest.get(app_id).cloned();

        // Find applicable release (respecting settings.allow_prereleases)
        let latest_release = card
            .releases
            .iter()
            .find(|r| self.settings.allow_prereleases || !r.prerelease)
            .cloned();

        // 1. Header with App Name, Tagline, and Status
        ui.horizontal(|ui| {
            let (icon, color) = match app_id {
                AppId::OxyBrush => ("🖌", Color32::from_rgb(56, 189, 248)),
                AppId::OxyPixel => ("▦", Color32::from_rgb(234, 179, 8)),
            };

            ui.label(RichText::new(icon).size(36.0).color(color));
            ui.add_space(10.0);

            ui.vertical(|ui| {
                ui.heading(
                    RichText::new(app_id.display_name())
                        .size(24.0)
                        .color(Color32::WHITE)
                        .strong(),
                );
                ui.label(
                    RichText::new(app_id.tagline())
                        .size(13.0)
                        .color(Color32::from_rgb(160, 175, 195)),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(inst) = &installed {
                    let has_update = latest_release.as_ref().is_some_and(|r| {
                        if let Ok(latest_ver) = r.version() {
                            latest_ver > inst.version
                        } else {
                            false
                        }
                    });

                    if has_update {
                        ui.colored_label(
                            Color32::from_rgb(234, 179, 8),
                            RichText::new("⚡ Update Available").strong(),
                        );
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(34, 197, 94),
                            RichText::new(format!("✓ Installed ({})", inst.version)).strong(),
                        );
                    }
                } else {
                    ui.colored_label(
                        Color32::from_rgb(140, 150, 170),
                        RichText::new("Not Installed"),
                    );
                }
            });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // 2. App Description & Features
        ui.label(
            RichText::new(app_id.description())
                .size(13.5)
                .color(Color32::from_rgb(200, 210, 225)),
        );

        ui.add_space(20.0);

        // 3. Status messages or errors
        if let Some(err) = &card.error {
            ui.label(
                RichText::new(format!("⚠ {}", err))
                    .color(Color32::from_rgb(239, 68, 68))
                    .size(13.0),
            );
            ui.add_space(8.0);
        } else if let Some(msg) = &card.status_message {
            ui.label(
                RichText::new(format!("ℹ {}", msg))
                    .color(Color32::from_rgb(56, 189, 248))
                    .size(13.0),
            );
            ui.add_space(8.0);
        }

        // 4. Progress bar if installing
        if card.is_installing {
            if let Some(progress) = &card.progress {
                let frac = match (progress.bytes_downloaded, progress.total_bytes) {
                    (b, Some(tot)) if tot > 0 => (b as f32 / tot as f32).clamp(0.0, 1.0),
                    _ => 0.0,
                };

                ui.add(egui::ProgressBar::new(frac).show_percentage().animate(true));
                ui.label(
                    RichText::new(&progress.message)
                        .size(12.0)
                        .color(Color32::from_rgb(180, 190, 210)),
                );
            } else {
                ui.spinner();
            }
            ui.add_space(16.0);
        }

        // 5. Version Card & Actions
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if let Some(inst) = &installed {
                        ui.label(
                            RichText::new(format!("Installed Version: {}", inst.version))
                                .size(13.0)
                                .color(Color32::WHITE),
                        );
                    } else {
                        ui.label(
                            RichText::new("Installed Version: None")
                                .size(13.0)
                                .color(Color32::from_rgb(150, 160, 180)),
                        );
                    }

                    if let Some(rel) = &latest_release {
                        ui.label(
                            RichText::new(format!(
                                "Latest Release: {} {}",
                                rel.tag_name,
                                if rel.prerelease { "(Pre-release)" } else { "" }
                            ))
                            .size(12.0)
                            .color(Color32::from_rgb(56, 189, 248)),
                        );
                    } else if card.is_fetching {
                        ui.label(
                            RichText::new("Checking for releases...")
                                .size(12.0)
                                .color(Color32::from_rgb(140, 150, 170)),
                        );
                    } else {
                        ui.label(
                            RichText::new("No compatible releases found")
                                .size(12.0)
                                .color(Color32::from_rgb(200, 120, 120)),
                        );
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(rel) = &latest_release {
                        if ui.button("Release Notes").clicked() {
                            self.release_notes_modal = Some((app_id, rel.clone()));
                        }
                    }
                });
            });

            ui.add_space(16.0);

            // Action Buttons
            ui.horizontal(|ui| {
                let has_update = match (&installed, &latest_release) {
                    (Some(inst), Some(rel)) => {
                        if let Ok(v) = rel.version() {
                            v > inst.version
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if card.is_installing {
                    ui.add_enabled(
                        false,
                        egui::Button::new(RichText::new("Installing...").size(15.0).strong()),
                    );
                } else if let Some(_inst) = &installed {
                    // Launch Button (Primary)
                    let launch_btn = egui::Button::new(
                        RichText::new("🚀 Launch App")
                            .color(Color32::WHITE)
                            .size(15.0)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(34, 197, 94))
                    .min_size(vec2(140.0, 38.0));

                    if ui.add(launch_btn).clicked() {
                        self.launch_installed(app_id);
                    }

                    // If update available, show Update button
                    if has_update {
                        if let Some(rel) = latest_release.clone() {
                            let update_btn = egui::Button::new(
                                RichText::new(format!("⬆ Update to {}", rel.tag_name))
                                    .color(Color32::WHITE)
                                    .size(14.0),
                            )
                            .fill(Color32::from_rgb(56, 189, 248))
                            .min_size(vec2(140.0, 38.0));

                            if ui.add(update_btn).clicked() {
                                self.start_install(app_id, rel);
                            }
                        }
                    }

                    // Directory button
                    if ui.button("📂 Open Folder").clicked() {
                        if let Some(inst) = &installed {
                            Self::open_directory(&inst.install_dir);
                        }
                    }

                    // Uninstall button
                    if ui
                        .button(RichText::new("Uninstall").color(Color32::from_rgb(239, 68, 68)))
                        .clicked()
                    {
                        self.start_uninstall(app_id);
                    }
                } else if let Some(rel) = latest_release.clone() {
                    // Install Button
                    let install_btn = egui::Button::new(
                        RichText::new(format!("📥 Install {}", rel.tag_name))
                            .color(Color32::WHITE)
                            .size(15.0)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(56, 189, 248))
                    .min_size(vec2(160.0, 40.0));

                    if ui.add(install_btn).clicked() {
                        self.start_install(app_id, rel);
                    }
                } else {
                    ui.add_enabled(
                        false,
                        egui::Button::new(RichText::new("Unavailable").size(14.0)),
                    );
                }
            });

            ui.add_space(6.0);
        });

        // 6. Recent Releases History Accordion
        if !card.releases.is_empty() {
            ui.add_space(20.0);
            ui.collapsing(
                RichText::new("All Available Releases").size(14.0).strong(),
                |ui| {
                    for rel in &card.releases {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&rel.tag_name).monospace().strong().size(13.0));

                            if rel.prerelease {
                                ui.colored_label(
                                    Color32::from_rgb(234, 179, 8),
                                    RichText::new("pre-release").size(11.0),
                                );
                            }

                            if let Some(date) = &rel.published_at {
                                ui.label(
                                    RichText::new(date)
                                        .size(11.0)
                                        .color(Color32::from_rgb(130, 140, 160)),
                                );
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if !card.is_installing
                                    && ui.button("Install this version").clicked()
                                {
                                    let rel_clone = rel.clone();
                                    self.start_install(app_id, rel_clone);
                                }
                            });
                        });
                        ui.separator();
                    }
                },
            );
        }
    }
}
