use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use egui::{vec2, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};
use oxystudio_core::{
    AppDetector, AppId, DetectedApp, GitHubClient, InstallProgress, Installer, Launcher, Manifest,
    Release, Version,
};
use serde::{Deserialize, Serialize};

// ── BozNetwork Portal / Liquid Glass Design Tokens ──
const COLOR_BG: Color32 = Color32::from_rgb(7, 14, 26); // #070e1a
const COLOR_BG_APPBAR: Color32 = Color32::from_rgb(10, 21, 40); // #0a1528
const COLOR_BG_CARD: Color32 = Color32::from_rgb(16, 36, 64); // rgba(16, 36, 64, 0.45)
const COLOR_BG_CARD_HOVER: Color32 = Color32::from_rgb(22, 48, 82);
const COLOR_BORDER: Color32 = Color32::from_rgba_premultiplied(160, 190, 220, 32);
const COLOR_BORDER_STRONG: Color32 = Color32::from_rgba_premultiplied(160, 190, 220, 64);

const COLOR_TEXT: Color32 = Color32::from_rgb(238, 243, 249); // #eef3f9
const COLOR_MUTED: Color32 = Color32::from_rgb(158, 180, 201); // #9eb4c9
const COLOR_DIM: Color32 = Color32::from_rgb(109, 132, 153); // #6d8499

const COLOR_BLUE_BRIGHT: Color32 = Color32::from_rgb(43, 143, 212); // #2b8fd4 (Boz Blue)
const COLOR_ORANGE_BRIGHT: Color32 = Color32::from_rgb(240, 138, 26); // #f08a1a (Boz Orange)
const COLOR_GREEN_STATUS: Color32 = Color32::from_rgb(61, 155, 122); // #3d9b7a
const COLOR_RED_STATUS: Color32 = Color32::from_rgb(196, 92, 74); // #c45c4a

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalTab {
    Today,
    Apps,
    OxyBrush,
    OxyPixel,
    Settings,
}

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
    AppDetected {
        app_id: AppId,
        detected: DetectedApp,
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
    current_tab: PortalTab,
    settings: LauncherSettings,
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
            current_tab: PortalTab::Today,
            settings,
            apps,
            manifest,
            installer,
            tx,
            rx,
            release_notes_modal: None,
            global_status: None,
        };

        // Trigger initial releases fetch and system auto-discovery
        app.refresh_all_releases();
        app.scan_system_for_installed_apps();

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

    fn scan_system_for_installed_apps(&mut self) {
        self.global_status = Some("Scanning system for installed Oxy applications...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            for &app_id in AppId::ALL {
                if let Some(detected) = AppDetector::detect(app_id) {
                    let _ = tx.send(LauncherEvent::AppDetected { app_id, detected });
                }
            }
        });
    }

    fn locate_app_manually(&mut self, app_id: AppId) {
        let dialog = rfd::FileDialog::new().set_title(format!("Locate {}", app_id.display_name()));

        #[cfg(target_os = "windows")]
        let dialog = dialog.add_filter("Executable", &["exe"]);

        if let Some(path) = dialog.pick_file() {
            if path.is_file() {
                let install_dir = path.parent().unwrap_or(&path).to_path_buf();
                let version = AppDetector::probe_version(&path)
                    .unwrap_or_else(|| Version::new(0, 0, 0, Some("manual".to_string())));

                self.manifest.register(
                    app_id,
                    version.clone(),
                    "manually located".to_string(),
                    path.clone(),
                    install_dir,
                );
                let _ = self.installer.save_manifest(&self.manifest);

                if let Some(card) = self.apps.get_mut(&app_id) {
                    card.status_message = Some(format!(
                        "Linked executable: {} (v{})",
                        path.display(),
                        version
                    ));
                }
            }
        }
    }

    fn unlink_app(&mut self, app_id: AppId) {
        self.manifest.remove(app_id);
        let _ = self.installer.save_manifest(&self.manifest);
        if let Some(card) = self.apps.get_mut(&app_id) {
            card.status_message = Some("Unlinked application from launcher.".to_string());
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
                LauncherEvent::AppDetected { app_id, detected } => {
                    let should_register = match self.manifest.get(app_id) {
                        Some(existing) => !existing.is_valid(),
                        None => true,
                    };

                    if should_register {
                        let version = detected
                            .version
                            .clone()
                            .unwrap_or_else(|| Version::new(0, 0, 0, Some("detected".to_string())));
                        let tag = format!("detected ({})", detected.source);
                        self.manifest.register(
                            app_id,
                            version,
                            tag,
                            detected.executable_path.clone(),
                            detected.install_dir.clone(),
                        );
                        let _ = self.installer.save_manifest(&self.manifest);

                        if let Some(card) = self.apps.get_mut(&app_id) {
                            card.status_message = Some(format!(
                                "Located {} via {}",
                                app_id.display_name(),
                                detected.source
                            ));
                        }
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

        let any_busy = self.apps.values().any(|c| c.is_installing || c.is_fetching);
        if any_busy {
            ctx.request_repaint();
        }

        // Apply BozNetwork Liquid Glass dark navy theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = COLOR_BG;
        visuals.window_fill = COLOR_BG_APPBAR;
        visuals.widgets.noninteractive.bg_fill = COLOR_BG_CARD;
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(18, 38, 68);
        visuals.widgets.hovered.bg_fill = COLOR_BG_CARD_HOVER;
        visuals.widgets.active.bg_fill = Color32::from_rgb(28, 60, 102);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(16);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(16);
        visuals.widgets.active.corner_radius = CornerRadius::same(16);
        ctx.set_visuals(visuals);

        // ── Top Glass Appbar with Capsule Tabs ──
        egui::TopBottomPanel::top("appbar")
            .frame(
                Frame::side_top_panel(&ctx.style())
                    .fill(COLOR_BG_APPBAR)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Brand Mark & Title
                    ui.label(
                        RichText::new("✦")
                            .color(COLOR_BLUE_BRIGHT)
                            .size(24.0)
                            .strong(),
                    );
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading(
                                RichText::new("OXY STUDIO")
                                    .color(COLOR_TEXT)
                                    .size(17.0)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new("PORTAL")
                                    .color(COLOR_ORANGE_BRIGHT)
                                    .size(12.0)
                                    .strong(),
                            );
                        });
                        ui.label(
                            RichText::new("Creative Suite & Tools Hub")
                                .color(COLOR_MUTED)
                                .size(11.0),
                        );
                    });

                    ui.add_space(28.0);

                    // Capsule Navigation Chips (Boz App Store style)
                    self.draw_capsule_tabs(ui);

                    // Right utility actions
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new("⚙ Settings").size(12.5).color(
                                if self.current_tab == PortalTab::Settings {
                                    COLOR_BLUE_BRIGHT
                                } else {
                                    COLOR_MUTED
                                },
                            ))
                            .clicked()
                        {
                            self.current_tab = PortalTab::Settings;
                        }

                        if ui
                            .button(
                                RichText::new("🔍 Scan System")
                                    .size(12.5)
                                    .color(COLOR_MUTED),
                            )
                            .clicked()
                        {
                            self.scan_system_for_installed_apps();
                        }

                        let is_busy = self.apps.values().any(|c| c.is_fetching);
                        let refresh_btn = if is_busy {
                            ui.add_enabled(
                                false,
                                egui::Button::new(
                                    RichText::new("⟳ Checking...").size(12.5).color(COLOR_DIM),
                                ),
                            )
                        } else {
                            ui.button(
                                RichText::new("⟳ Refresh")
                                    .size(12.5)
                                    .color(COLOR_BLUE_BRIGHT),
                            )
                        };

                        if refresh_btn.clicked() {
                            self.refresh_all_releases();
                        }
                    });
                });
            });

        // ── Bottom Status Bar ──
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                Frame::side_top_panel(&ctx.style())
                    .fill(Color32::from_rgb(5, 10, 18))
                    .inner_margin(Margin::symmetric(24, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(status) = &self.global_status {
                        ui.label(RichText::new(status).color(COLOR_BLUE_BRIGHT).size(11.0));
                    } else {
                        let installed_count = self.manifest.apps.len();
                        ui.label(
                            RichText::new(format!(
                                "BozNetwork Portal Engine • {} app{} connected",
                                installed_count,
                                if installed_count == 1 { "" } else { "s" }
                            ))
                            .color(COLOR_DIM)
                            .size(11.0),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("OxyStudio v0.1.0")
                                .color(COLOR_DIM)
                                .size(11.0),
                        );
                    });
                });
            });

        // ── Main Content Area ──
        egui::CentralPanel::default()
            .frame(
                Frame::central_panel(&ctx.style())
                    .fill(COLOR_BG)
                    .inner_margin(Margin::same(28)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match self.current_tab {
                    PortalTab::Today => self.show_today_view(ui),
                    PortalTab::Apps => self.show_apps_view(ui),
                    PortalTab::OxyBrush => self.show_app_page(ui, AppId::OxyBrush),
                    PortalTab::OxyPixel => self.show_app_page(ui, AppId::OxyPixel),
                    PortalTab::Settings => self.show_settings_view(ui),
                });
            });

        // Release Notes Modal
        if let Some((app_id, release)) = self.release_notes_modal.clone() {
            let mut open = true;
            egui::Window::new(format!(
                "Release Notes: {} {}",
                app_id.display_name(),
                release.tag_name
            ))
            .open(&mut open)
            .min_width(540.0)
            .max_height(480.0)
            .show(ctx, |ui| {
                ui.heading(release.display_title());
                if let Some(published) = &release.published_at {
                    ui.label(
                        RichText::new(format!("Published: {}", published))
                            .size(11.5)
                            .color(COLOR_MUTED),
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
    fn draw_capsule_tabs(&mut self, ui: &mut egui::Ui) {
        let tabs = [
            (PortalTab::Today, "Today"),
            (PortalTab::Apps, "All Tools"),
            (PortalTab::OxyBrush, "OxyBrush"),
            (PortalTab::OxyPixel, "OxyPixel"),
        ];

        for (tab, label) in tabs {
            let is_active = self.current_tab == tab;
            let (bg, text_col) = if is_active {
                (COLOR_BLUE_BRIGHT, Color32::WHITE)
            } else {
                (
                    Color32::from_rgba_premultiplied(18, 38, 68, 140),
                    COLOR_MUTED,
                )
            };

            let btn = egui::Button::new(RichText::new(label).size(13.0).color(text_col).strong())
                .fill(bg)
                .corner_radius(CornerRadius::same(14))
                .min_size(vec2(76.0, 28.0));

            if ui.add(btn).clicked() {
                self.current_tab = tab;
            }
        }
    }

    /// App Store "Today" Editorial View with Hero Card
    fn show_today_view(&mut self, ui: &mut egui::Ui) {
        // Editorial Eyebrow & Headline
        ui.label(
            RichText::new("TODAY • OXY CREATIVE PORTAL")
                .color(COLOR_DIM)
                .size(11.0)
                .strong(),
        );
        ui.heading(
            RichText::new("Discover & Launch")
                .color(COLOR_TEXT)
                .size(32.0)
                .strong(),
        );

        ui.add_space(16.0);

        // Hero Story Card (App Store style)
        self.draw_hero_card(ui, AppId::OxyBrush);

        ui.add_space(28.0);

        // Secondary Story Card
        ui.label(
            RichText::new("FEATURED STUDIO TOOL")
                .color(COLOR_DIM)
                .size(11.0)
                .strong(),
        );
        ui.add_space(6.0);
        self.draw_hero_card(ui, AppId::OxyPixel);
    }

    /// All Tools Grid View
    fn show_apps_view(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("OXY TOOLS REGISTRY")
                .color(COLOR_DIM)
                .size(11.0)
                .strong(),
        );
        ui.heading(
            RichText::new("Installed & Available Tools")
                .color(COLOR_TEXT)
                .size(28.0)
                .strong(),
        );

        ui.add_space(20.0);

        for &app_id in AppId::ALL {
            self.draw_app_row_card(ui, app_id);
            ui.add_space(14.0);
        }
    }

    /// Dedicated Tool Detail Page
    fn show_app_page(&mut self, ui: &mut egui::Ui, app_id: AppId) {
        self.draw_hero_card(ui, app_id);

        ui.add_space(24.0);
        let card = self.apps.get(&app_id).cloned().unwrap_or_default();

        if !card.releases.is_empty() {
            ui.label(
                RichText::new("VERSION HISTORY & RELEASES")
                    .color(COLOR_DIM)
                    .size(11.0)
                    .strong(),
            );
            ui.add_space(8.0);

            for rel in &card.releases {
                Frame::NONE
                    .fill(COLOR_BG_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&rel.tag_name)
                                    .color(COLOR_TEXT)
                                    .monospace()
                                    .strong()
                                    .size(13.0),
                            );
                            if rel.prerelease {
                                ui.colored_label(
                                    COLOR_ORANGE_BRIGHT,
                                    RichText::new("pre-release").size(11.0),
                                );
                            }
                            if let Some(date) = &rel.published_at {
                                ui.label(RichText::new(date).size(11.0).color(COLOR_DIM));
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
                    });
                ui.add_space(6.0);
            }
        }
    }

    /// BozNetwork Liquid Glass Hero Card
    fn draw_hero_card(&mut self, ui: &mut egui::Ui, app_id: AppId) {
        let card = self.apps.get(&app_id).cloned().unwrap_or_default();
        let installed = self.manifest.get(app_id).cloned();

        let latest_release = card
            .releases
            .iter()
            .find(|r| self.settings.allow_prereleases || !r.prerelease)
            .cloned();

        Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER_STRONG))
            .corner_radius(CornerRadius::same(20))
            .inner_margin(Margin::same(24))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: App Icon & Identity
                    let (icon_symbol, accent_color) = match app_id {
                        AppId::OxyBrush => ("🖌", COLOR_BLUE_BRIGHT),
                        AppId::OxyPixel => ("▦", COLOR_ORANGE_BRIGHT),
                    };

                    Frame::NONE
                        .fill(COLOR_BG_APPBAR)
                        .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                        .corner_radius(CornerRadius::same(18))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(RichText::new(icon_symbol).size(38.0).color(accent_color));
                        });

                    ui.add_space(16.0);

                    // Center: App Info & Story
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(match app_id {
                                AppId::OxyBrush => "3D DIGITAL SCULPTING & TOPOLOGY",
                                AppId::OxyPixel => "PIXEL ART, TILESETS & ANIMATION",
                            })
                            .color(COLOR_DIM)
                            .size(11.0)
                            .strong(),
                        );

                        ui.heading(
                            RichText::new(app_id.display_name())
                                .size(24.0)
                                .color(COLOR_TEXT)
                                .strong(),
                        );

                        ui.label(
                            RichText::new(app_id.tagline())
                                .size(13.0)
                                .color(COLOR_MUTED),
                        );
                    });

                    // Right: App Store Capsule Pill Button
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.draw_capsule_action_button(
                            ui,
                            app_id,
                            &installed,
                            &latest_release,
                            &card,
                        );
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);

                // Body Description
                ui.label(
                    RichText::new(app_id.description())
                        .size(13.0)
                        .color(COLOR_MUTED),
                );

                ui.add_space(14.0);

                // Progress Bar if installing
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
                                .color(COLOR_BLUE_BRIGHT),
                        );
                    } else {
                        ui.spinner();
                    }
                    ui.add_space(10.0);
                }

                // Status message or location details
                ui.horizontal(|ui| {
                    if let Some(inst) = &installed {
                        ui.label(
                            RichText::new("● INSTALLED")
                                .color(COLOR_GREEN_STATUS)
                                .size(11.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("v{}", inst.version))
                                .color(COLOR_TEXT)
                                .size(11.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("({})", inst.executable_path.display()))
                                .color(COLOR_DIM)
                                .monospace()
                                .size(10.5),
                        );
                    } else {
                        ui.label(
                            RichText::new("○ NOT INSTALLED")
                                .color(COLOR_DIM)
                                .size(11.0)
                                .strong(),
                        );
                        if let Some(rel) = &latest_release {
                            ui.label(
                                RichText::new(format!("Latest: {}", rel.tag_name))
                                    .color(COLOR_BLUE_BRIGHT)
                                    .size(11.0),
                            );
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if installed.is_some() {
                            if ui
                                .button(RichText::new("📂 Open Folder").size(11.5))
                                .clicked()
                            {
                                if let Some(inst) = &installed {
                                    Self::open_directory(&inst.install_dir);
                                }
                            }

                            if ui.button(RichText::new("🔍 Relocate").size(11.5)).clicked() {
                                self.locate_app_manually(app_id);
                            }

                            if let Some(inst) = &installed {
                                let is_managed = !inst.tag.starts_with("detected")
                                    && !inst.tag.starts_with("manual");
                                if is_managed {
                                    if ui
                                        .button(
                                            RichText::new("Uninstall")
                                                .color(COLOR_RED_STATUS)
                                                .size(11.5),
                                        )
                                        .clicked()
                                    {
                                        self.start_uninstall(app_id);
                                    }
                                } else if ui
                                    .button(
                                        RichText::new("Unlink").color(COLOR_RED_STATUS).size(11.5),
                                    )
                                    .clicked()
                                {
                                    self.unlink_app(app_id);
                                }
                            }
                        } else {
                            if ui
                                .button(RichText::new("🔍 Locate Existing App").size(11.5))
                                .clicked()
                            {
                                self.locate_app_manually(app_id);
                            }
                        }

                        if let Some(rel) = &latest_release {
                            if ui.button(RichText::new("Notes").size(11.5)).clicked() {
                                self.release_notes_modal = Some((app_id, rel.clone()));
                            }
                        }
                    });
                });
            });
    }

    /// Inset Grouped Row Card for All Tools List
    fn draw_app_row_card(&mut self, ui: &mut egui::Ui, app_id: AppId) {
        let card = self.apps.get(&app_id).cloned().unwrap_or_default();
        let installed = self.manifest.get(app_id).cloned();
        let latest_release = card
            .releases
            .iter()
            .find(|r| self.settings.allow_prereleases || !r.prerelease)
            .cloned();

        Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let icon_sym = match app_id {
                        AppId::OxyBrush => "🖌",
                        AppId::OxyPixel => "▦",
                    };
                    ui.label(RichText::new(icon_sym).size(26.0).color(COLOR_BLUE_BRIGHT));
                    ui.add_space(10.0);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(app_id.display_name())
                                    .size(16.0)
                                    .color(COLOR_TEXT)
                                    .strong(),
                            );
                            if let Some(inst) = &installed {
                                ui.label(
                                    RichText::new(format!("v{}", inst.version))
                                        .color(COLOR_GREEN_STATUS)
                                        .size(12.0)
                                        .strong(),
                                );
                            }
                        });
                        ui.label(
                            RichText::new(app_id.tagline())
                                .size(12.0)
                                .color(COLOR_MUTED),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.draw_capsule_action_button(
                            ui,
                            app_id,
                            &installed,
                            &latest_release,
                            &card,
                        );
                    });
                });
            });
    }

    /// App Store Pill / Capsule Button (GET / OPEN / UPDATE)
    fn draw_capsule_action_button(
        &mut self,
        ui: &mut egui::Ui,
        app_id: AppId,
        installed: &Option<oxystudio_core::InstalledApp>,
        latest_release: &Option<Release>,
        card: &AppCardState,
    ) {
        let has_update = match (installed, latest_release) {
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
                egui::Button::new(RichText::new("INSTALLING").size(13.0).strong())
                    .corner_radius(CornerRadius::same(16))
                    .min_size(vec2(100.0, 34.0)),
            );
        } else if let Some(_inst) = installed {
            if has_update {
                if let Some(rel) = latest_release.clone() {
                    let btn = egui::Button::new(
                        RichText::new("UPDATE")
                            .color(Color32::WHITE)
                            .size(13.0)
                            .strong(),
                    )
                    .fill(COLOR_ORANGE_BRIGHT)
                    .corner_radius(CornerRadius::same(16))
                    .min_size(vec2(92.0, 34.0));

                    if ui.add(btn).clicked() {
                        self.start_install(app_id, rel);
                    }
                }
            }

            let open_btn = egui::Button::new(
                RichText::new("OPEN")
                    .color(Color32::WHITE)
                    .size(13.0)
                    .strong(),
            )
            .fill(COLOR_BLUE_BRIGHT)
            .corner_radius(CornerRadius::same(16))
            .min_size(vec2(86.0, 34.0));

            if ui.add(open_btn).clicked() {
                self.launch_installed(app_id);
            }
        } else if let Some(rel) = latest_release.clone() {
            let get_btn = egui::Button::new(
                RichText::new("GET")
                    .color(Color32::WHITE)
                    .size(13.0)
                    .strong(),
            )
            .fill(Color32::from_rgb(18, 60, 110))
            .stroke(Stroke::new(1.0_f32, COLOR_BLUE_BRIGHT))
            .corner_radius(CornerRadius::same(16))
            .min_size(vec2(86.0, 34.0));

            if ui.add(get_btn).clicked() {
                self.start_install(app_id, rel);
            }
        } else {
            ui.add_enabled(
                false,
                egui::Button::new(RichText::new("GET").size(13.0))
                    .corner_radius(CornerRadius::same(16)),
            );
        }
    }

    /// Settings View
    fn show_settings_view(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("CONFIGURATION & PREFERENCES")
                .color(COLOR_DIM)
                .size(11.0)
                .strong(),
        );
        ui.heading(
            RichText::new("Portal Settings")
                .color(COLOR_TEXT)
                .size(28.0)
                .strong(),
        );

        ui.add_space(20.0);

        Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(Margin::same(24))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Release Channels")
                        .size(16.0)
                        .color(COLOR_TEXT)
                        .strong(),
                );
                ui.add_space(6.0);
                if ui
                    .checkbox(
                        &mut self.settings.allow_prereleases,
                        "Include Pre-releases (Alphas, Betas, Release Candidates)",
                    )
                    .changed()
                {
                    self.settings.save();
                }

                ui.add_space(14.0);
                if ui
                    .checkbox(
                        &mut self.settings.auto_check_updates,
                        "Check for updates automatically on start",
                    )
                    .changed()
                {
                    self.settings.save();
                }

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(16.0);

                ui.label(
                    RichText::new("Managed Installation Directory")
                        .size(16.0)
                        .color(COLOR_TEXT)
                        .strong(),
                );
                ui.add_space(6.0);
                let (default_apps, _) = Installer::default_paths();
                let current_dir = self
                    .settings
                    .custom_apps_dir
                    .clone()
                    .unwrap_or(default_apps);

                ui.label(
                    RichText::new(current_dir.display().to_string())
                        .color(COLOR_MUTED)
                        .monospace()
                        .size(12.0),
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("📂 Open Apps Folder").clicked() {
                        Self::open_directory(&current_dir);
                    }

                    if ui.button("🔍 Run System Auto-Discovery").clicked() {
                        self.scan_system_for_installed_apps();
                    }
                });
            });
    }
}
