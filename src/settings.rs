use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::commands::{MagicWord, StartMode};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HotkeyConfig {
    pub super_key: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            super_key: true,
            ctrl: false,
            shift: true,
            alt: false,
            key: "Q".into(),
        }
    }
}

impl HotkeyConfig {
    pub fn to_hotkey(&self) -> Option<HotKey> {
        let code = key_name_to_code(&self.key)?;
        let mut mods = Modifiers::empty();
        if self.super_key {
            mods |= Modifiers::SUPER;
        }
        if self.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if self.shift {
            mods |= Modifiers::SHIFT;
        }
        if self.alt {
            mods |= Modifiers::ALT;
        }
        let mods_opt = if mods.is_empty() { None } else { Some(mods) };
        Some(HotKey::new(mods_opt, code))
    }

    pub fn display_string(&self) -> String {
        let mut parts = Vec::new();
        if self.super_key {
            parts.push("Win");
        }
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(&self.key);
        parts.join("+")
    }
}

pub const AVAILABLE_KEYS: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
    "S", "T", "U", "V", "W", "X", "Y", "Z", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8",
    "F9", "F10", "F11", "F12", "Space",
];

fn key_name_to_code(name: &str) -> Option<Code> {
    match name {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        "Space" => Some(Code::Space),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub font_color: [u8; 3],
    pub window_width: f32,
    pub window_height: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    pub hide_on_inactive: bool,
    pub opacity_percent: u8,
    pub magic_words: Vec<MagicWord>,
    #[serde(default)]
    pub hotkey: HotkeyConfig,
    #[serde(default = "default_true")]
    pub stay_on_top: bool,
    #[serde(default)]
    pub start_at_startup: bool,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,
}

fn default_font_size() -> f32 {
    16.0
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_color: [0, 200, 120],
            window_width: 400.0,
            window_height: 48.0,
            font_size: 16.0,
            hide_on_inactive: true,
            opacity_percent: 100,
            magic_words: vec![],
            hotkey: HotkeyConfig::default(),
            stay_on_top: true,
            start_at_startup: false,
            window_x: None,
            window_y: None,
        }
    }
}

impl Settings {
    pub fn config_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("slickrun");
        p.push("config.json");
        p
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn font_color32(&self) -> egui::Color32 {
        egui::Color32::from_rgb(self.font_color[0], self.font_color[1], self.font_color[2])
    }

    pub fn export_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("slickrun");
        p.push("magic_words.json");
        p
    }

    pub fn export_magic_words(words: &[MagicWord]) -> Result<PathBuf, String> {
        let path = Self::export_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(words).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn import_magic_words() -> Result<Vec<MagicWord>, String> {
        let path = Self::export_path();
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let words: Vec<MagicWord> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(words)
    }

    pub fn autostart_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("autostart");
        p.push("slickrun.desktop");
        p
    }

    pub fn update_autostart(&self) {
        let path = Self::autostart_path();
        if self.start_at_startup {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("slickrun"));
            let desktop = format!(
                "[Desktop Entry]\nType=Application\nName=SlickRun\nExec={}\nStartupNotify=false\nTerminal=false\n",
                exe.display()
            );
            let _ = std::fs::write(&path, desktop);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[derive(PartialEq, Clone)]
enum SettingsTab {
    Library,
    Appearance,
    Options,
}

pub struct SettingsWindow {
    pub open: bool,
    pub draft: Settings,
    tab: SettingsTab,
    // Magic word editing
    show_edit_form: bool,
    edit_keyword: String,
    edit_filename_or_url: String,
    edit_start_mode: StartMode,
    edit_start_path: String,
    edit_parameters: String,
    editing_index: Option<usize>,
    // Search filter
    search_filter: String,
    // Status message
    status_message: String,
}

impl SettingsWindow {
    pub fn new(settings: &Settings) -> Self {
        Self {
            open: false,
            draft: settings.clone(),
            tab: SettingsTab::Library,
            show_edit_form: false,
            edit_keyword: String::new(),
            edit_filename_or_url: String::new(),
            edit_start_mode: StartMode::Normal,
            edit_start_path: String::new(),
            edit_parameters: String::new(),
            editing_index: None,
            search_filter: String::new(),
            status_message: String::new(),
        }
    }

    pub fn open(&mut self, settings: &Settings) {
        self.draft = settings.clone();
        self.clear_edit_fields();
        self.tab = SettingsTab::Library;
        self.search_filter.clear();
        self.status_message.clear();
        self.open = true;
    }

    fn clear_edit_fields(&mut self) {
        self.show_edit_form = false;
        self.edit_keyword.clear();
        self.edit_filename_or_url.clear();
        self.edit_start_mode = StartMode::Normal;
        self.edit_start_path.clear();
        self.edit_parameters.clear();
        self.editing_index = None;
    }

    /// Draw settings UI into a child viewport's CentralPanel.
    /// Returns Some(settings) if user clicked Save & Close.
    pub fn show_in_viewport(&mut self, ctx: &egui::Context) -> Option<Settings> {
        let mut saved = None;

        // Use default dark visuals for the settings window
        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, SettingsTab::Library, "  Library  ");
                ui.selectable_value(
                    &mut self.tab,
                    SettingsTab::Appearance,
                    "  Appearance  ",
                );
                ui.selectable_value(&mut self.tab, SettingsTab::Options, "  Options  ");
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.tab {
                    SettingsTab::Library => self.show_library_tab(ui),
                    SettingsTab::Appearance => self.show_appearance_tab(ui),
                    SettingsTab::Options => self.show_options_tab(ui),
                }

                ui.add_space(12.0);

                // Status message
                if !self.status_message.is_empty() {
                    ui.label(
                        egui::RichText::new(&self.status_message)
                            .color(egui::Color32::YELLOW)
                            .small(),
                    );
                    ui.add_space(4.0);
                }

                ui.separator();
                if ui.button("Save & Close").clicked() {
                    saved = Some(self.draft.clone());
                    self.open = false;
                }
            });
        });

        // Handle OS window close button
        if ctx.input(|i| i.viewport().close_requested()) {
            self.open = false;
        }

        saved
    }

    fn show_library_tab(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.editing_index = None;
                self.edit_keyword.clear();
                self.edit_filename_or_url.clear();
                self.edit_start_mode = StartMode::Normal;
                self.edit_start_path.clear();
                self.edit_parameters.clear();
                self.show_edit_form = true;
            }

            let has_selection = self.editing_index.is_some();

            if ui
                .add_enabled(has_selection, egui::Button::new("Edit"))
                .clicked()
            {
                self.show_edit_form = true;
            }

            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                if let Some(idx) = self.editing_index {
                    self.draft.magic_words.remove(idx);
                    self.clear_edit_fields();
                }
            }

            ui.separator();

            ui.label("Find:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search_filter).desired_width(120.0),
            );

            ui.separator();

            if ui.button("Import").clicked() {
                match Settings::import_magic_words() {
                    Ok(words) => {
                        let count = words.len();
                        self.draft.magic_words = words;
                        self.status_message = format!("Imported {} magic words", count);
                    }
                    Err(e) => {
                        self.status_message = format!("Import failed: {}", e);
                    }
                }
            }
            if ui.button("Export").clicked() {
                match Settings::export_magic_words(&self.draft.magic_words) {
                    Ok(path) => {
                        self.status_message = format!("Exported to {}", path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Export failed: {}", e);
                    }
                }
            }
        });

        ui.add_space(8.0);

        // Table
        let filter_lower = self.search_filter.to_lowercase();
        let filtered_indices: Vec<usize> = self
            .draft
            .magic_words
            .iter()
            .enumerate()
            .filter(|(_, mw)| {
                filter_lower.is_empty()
                    || mw.keyword.to_lowercase().contains(&filter_lower)
                    || mw.filename_or_url.to_lowercase().contains(&filter_lower)
            })
            .map(|(i, _)| i)
            .collect();

        egui::Grid::new("magic_words_table")
            .striped(true)
            .num_columns(5)
            .min_col_width(60.0)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Keyword");
                ui.strong("Filename / URL");
                ui.strong("Mode");
                ui.strong("Start Path");
                ui.strong("Parameters");
                ui.end_row();

                for &idx in &filtered_indices {
                    let mw = &self.draft.magic_words[idx];
                    let selected = self.editing_index == Some(idx);

                    let resp = ui.selectable_label(selected, &mw.keyword);
                    if resp.clicked() {
                        let mw = &self.draft.magic_words[idx];
                        self.editing_index = Some(idx);
                        self.edit_keyword = mw.keyword.clone();
                        self.edit_filename_or_url = mw.filename_or_url.clone();
                        self.edit_start_mode = mw.start_mode.clone();
                        self.edit_start_path = mw.start_path.clone().unwrap_or_default();
                        self.edit_parameters = mw.parameters.clone().unwrap_or_default();
                        self.show_edit_form = false; // just select, don't open form
                    }
                    if resp.double_clicked() {
                        self.show_edit_form = true; // double-click opens edit form
                    }

                    ui.label(&mw.filename_or_url);
                    ui.label(match mw.start_mode {
                        StartMode::Normal => "Normal",
                        StartMode::Sudo => "Sudo",
                    });
                    ui.label(mw.start_path.as_deref().unwrap_or(""));
                    ui.label(mw.parameters.as_deref().unwrap_or(""));
                    ui.end_row();
                }
            });

        if filtered_indices.is_empty() && self.draft.magic_words.is_empty() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No magic words yet. Click \"New\" to add one.")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        } else if filtered_indices.is_empty() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No matches found.")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        }

        // Edit form — only shown when New or Edit is clicked
        if self.show_edit_form {
            ui.add_space(12.0);
            ui.separator();
            ui.heading(if self.editing_index.is_some() {
                "Edit Magic Word"
            } else {
                "New Magic Word"
            });

            egui::Grid::new("mw_edit_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Keyword:");
                    ui.text_edit_singleline(&mut self.edit_keyword);
                    ui.end_row();

                    ui.label("Filename / URL:");
                    ui.text_edit_singleline(&mut self.edit_filename_or_url);
                    ui.end_row();

                    ui.label("Start Mode:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.edit_start_mode, StartMode::Normal, "Normal");
                        ui.radio_value(&mut self.edit_start_mode, StartMode::Sudo, "Sudo");
                    });
                    ui.end_row();

                    ui.label("Start Path:");
                    ui.text_edit_singleline(&mut self.edit_start_path);
                    ui.end_row();

                    ui.label("Parameters:");
                    ui.text_edit_singleline(&mut self.edit_parameters);
                    ui.end_row();
                });

            ui.label(
                egui::RichText::new(
                    "Tip: Use $W$ for user arguments. Set Filename to @copy@ to copy params to clipboard.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );

            ui.horizontal(|ui| {
                let label = if self.editing_index.is_some() {
                    "Update"
                } else {
                    "Add"
                };
                if ui.button(label).clicked()
                    && !self.edit_keyword.is_empty()
                    && !self.edit_filename_or_url.is_empty()
                {
                    let mw = MagicWord {
                        keyword: self.edit_keyword.clone(),
                        filename_or_url: self.edit_filename_or_url.clone(),
                        start_mode: self.edit_start_mode.clone(),
                        start_path: if self.edit_start_path.is_empty() {
                            None
                        } else {
                            Some(self.edit_start_path.clone())
                        },
                        parameters: if self.edit_parameters.is_empty() {
                            None
                        } else {
                            Some(self.edit_parameters.clone())
                        },
                    };
                    if let Some(idx) = self.editing_index {
                        self.draft.magic_words[idx] = mw;
                    } else {
                        self.draft.magic_words.push(mw);
                    }
                    self.clear_edit_fields();
                }
                if ui.button("Cancel").clicked() {
                    self.clear_edit_fields();
                }
            });
        }
    }

    fn show_appearance_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Font color:");
            let mut color = self.draft.font_color;
            ui.color_edit_button_srgb(&mut color);
            self.draft.font_color = color;
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Font size:");
            ui.add(
                egui::DragValue::new(&mut self.draft.font_size)
                    .range(8.0..=48.0)
                    .speed(0.5)
                    .suffix(" px"),
            );
        });

        ui.add_space(12.0);
        ui.heading("Window Size");

        ui.horizontal(|ui| {
            ui.label("Width:");
            ui.add(
                egui::DragValue::new(&mut self.draft.window_width)
                    .range(200.0..=1200.0)
                    .speed(1.0)
                    .suffix(" px"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Height:");
            ui.add(
                egui::DragValue::new(&mut self.draft.window_height)
                    .range(32.0..=200.0)
                    .speed(1.0)
                    .suffix(" px"),
            );
        });
    }

    fn show_options_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);

        // Hotkey
        ui.heading("Hotkey");
        ui.label(format!(
            "Current: {}",
            self.draft.hotkey.display_string()
        ));

        ui.horizontal(|ui| {
            ui.label("Modifiers:");
            ui.checkbox(&mut self.draft.hotkey.super_key, "Win");
            ui.checkbox(&mut self.draft.hotkey.ctrl, "Ctrl");
            ui.checkbox(&mut self.draft.hotkey.shift, "Shift");
            ui.checkbox(&mut self.draft.hotkey.alt, "Alt");
        });

        ui.horizontal(|ui| {
            ui.label("Key:");
            egui::ComboBox::from_id_salt("hotkey_key")
                .selected_text(&self.draft.hotkey.key)
                .show_ui(ui, |ui| {
                    for &k in AVAILABLE_KEYS {
                        ui.selectable_value(&mut self.draft.hotkey.key, k.to_string(), k);
                    }
                });
        });

        ui.label(
            egui::RichText::new(
                "On Wayland, global hotkey may not work. Fallback: echo t > $XDG_RUNTIME_DIR/slickrun-toggle",
            )
            .small()
            .color(egui::Color32::GRAY),
        );

        ui.add_space(12.0);

        // Startup
        ui.heading("Startup");
        ui.checkbox(&mut self.draft.start_at_startup, "Start at login");

        ui.add_space(12.0);

        // Behavior
        ui.heading("Behavior");
        ui.checkbox(
            &mut self.draft.hide_on_inactive,
            "Autohide (hide when window loses focus)",
        );
        ui.checkbox(&mut self.draft.stay_on_top, "Stay on top of all windows");

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Opacity:");
            ui.add(
                egui::Slider::new(&mut self.draft.opacity_percent, 20..=100).suffix("%"),
            );
        });
        ui.label(
            egui::RichText::new("Lower opacity makes the desktop visible behind the window.")
                .small()
                .color(egui::Color32::GRAY),
        );
    }
}
