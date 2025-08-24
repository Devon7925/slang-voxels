use cgmath::{Point3, Vector3};
use egui_probe::EguiProbe;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, EguiProbe)]
pub enum WorldGenSettings {
    Normal,
    Control(u32),
}

impl Default for WorldGenSettings {
    fn default() -> Self {
        WorldGenSettings::Normal
    }
}
impl WorldGenSettings {
    pub fn get_name(&self) -> &str {
        match self {
            WorldGenSettings::Normal => "Normal",
            WorldGenSettings::Control(_) => "Control",
        }
    }

    pub fn get_seed(&self) -> u32 {
        match self {
            WorldGenSettings::Normal => 0,
            WorldGenSettings::Control(seed) => *seed,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GameModeSettings {
    PracticeRange { spawn_location: Point3<f32> },
    Explorer { spawn_location: Point3<f32> },
    FFA,
    Control,
}

// keep default impl if needed
impl Default for GameModeSettings {
    fn default() -> Self {
        GameModeSettings::FFA
    }
}
impl GameModeSettings {
    pub fn get_name(&self) -> &str {
        match self {
            GameModeSettings::PracticeRange { .. } => "Practice Range",
            GameModeSettings::Explorer { .. } => "Explorer",
            GameModeSettings::FFA => "FFA",
            GameModeSettings::Control => "Control",
        }
    }
}

impl egui_probe::EguiProbe for GameModeSettings {
    fn probe(&mut self, ui: &mut egui::Ui, _style: &egui_probe::Style) -> egui::Response {
        ui.vertical(|ui| {
            ui.label("Game mode");
            let resp = egui::ComboBox::from_label("")
                .selected_text(match self {
                    GameModeSettings::PracticeRange { .. } => "PracticeRange".to_string(),
                    GameModeSettings::Explorer { .. } => "Explorer".to_string(),
                    GameModeSettings::FFA => "FFA".to_string(),
                    GameModeSettings::Control => "Control".to_string(),
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            matches!(self, GameModeSettings::PracticeRange { .. }),
                            "PracticeRange",
                        )
                        .clicked()
                    {
                        if !matches!(self, GameModeSettings::PracticeRange { .. }) {
                            *self = GameModeSettings::PracticeRange {
                                spawn_location: Point3::new(0.0, 0.0, 0.0),
                            };
                        }
                    }
                    if ui
                        .selectable_label(
                            matches!(self, GameModeSettings::Explorer { .. }),
                            "Explorer",
                        )
                        .clicked()
                    {
                        if !matches!(self, GameModeSettings::Explorer { .. }) {
                            *self = GameModeSettings::Explorer {
                                spawn_location: Point3::new(0.0, 0.0, 0.0),
                            };
                        }
                    }
                    if ui
                        .selectable_label(matches!(self, GameModeSettings::FFA), "FFA")
                        .clicked()
                    {
                        *self = GameModeSettings::FFA;
                    }
                    if ui
                        .selectable_label(matches!(self, GameModeSettings::Control), "Control")
                        .clicked()
                    {
                        *self = GameModeSettings::Control;
                    }
                })
                .response
                .clone();

            resp
        })
        .response
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, EguiProbe)]
pub struct GameSettings {
    pub name: String,
    pub delta_time: f32,
    pub is_remote: bool,
    pub rollback_buffer_size: u32,
    pub player_count: u32,
    #[egui_probe(skip)]
    pub render_size: Vector3<u32>,
    pub max_loaded_chunks: u32,
    pub max_worldgen_rate: u32,
    pub max_update_rate: u32,
    pub world_gen: WorldGenSettings,
    pub game_mode: GameModeSettings,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            name: String::new(),
            delta_time: 0.0,
            is_remote: false,
            rollback_buffer_size: 0,
            player_count: 1,
            render_size: Vector3::new(64, 64, 64),
            max_loaded_chunks: 0,
            max_worldgen_rate: 0,
            max_update_rate: 0,
            world_gen: WorldGenSettings::default(),
            game_mode: GameModeSettings::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub String);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Lobby {
    pub name: String,
    pub player_count: u32,
    pub lobby_id: RoomId,
    pub settings: GameSettings,
}