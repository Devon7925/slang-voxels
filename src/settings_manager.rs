use std::fmt::Display;

use egui::{self, ComboBox};
use egui_probe::EguiProbe;
use serde::{Deserialize, Serialize};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::{playground_module, shared::GameSettings};

#[derive(Debug, Deserialize, Serialize, Clone, EguiProbe)]
pub struct Settings {
    pub local_url: String,
    pub remote_url: String,
    pub card_file: String,
    pub card_dir: String,
    #[egui_probe(skip)]
    pub fullscreen_toggle: KeyCode,
    pub movement_controls: ControlSettings,
    pub graphics_settings: playground_module::GraphicsSettings,
    pub replay_settings: ReplaySettings,
    pub do_profiling: bool,
    pub crash_log: String,
    pub preset_settings: Vec<GameSettings>,
    pub create_lobby_settings: GameSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum Control {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl Display for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Control::Key(key) => write!(f, "{:?}", key),
            Control::Mouse(MouseButton::Left) => write!(f, "↖"),
            Control::Mouse(MouseButton::Middle) => write!(f, "⬆"),
            Control::Mouse(MouseButton::Right) => write!(f, "↗"),
            Control::Mouse(button) => write!(f, "{:?}", button),
        }
    }
}

impl Settings {
    pub fn from_string(yaml_string: &str) -> Self {
        serde_yml::from_str(yaml_string).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_yml::to_string(self).unwrap()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, EguiProbe)]
pub struct ControlSettings {
    pub forward: Control,
    pub backward: Control,
    pub left: Control,
    pub right: Control,
    pub jump: Control,
    pub crouch: Control,
    pub sensitivity: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone, EguiProbe)]
pub struct ReplaySettings {
    pub replay_folder: String,
    pub record_replay: bool,
}

// Helper used by egui-probe derive to render ControlSettings
// We implement a custom probe for the Control enum below so each control field
// will render a ComboBox selector when probed.

impl EguiProbe for Control {
    fn probe(&mut self, ui: &mut egui::Ui, _style: &egui_probe::Style) -> egui::Response {
        let mut current = match self {
            Control::Key(k) => format!("Key::{:?}", k),
            Control::Mouse(b) => format!("Mouse::{:?}", b),
        };

        let resp = ComboBox::from_label("")
            .selected_text(current.clone())
            .show_ui(ui, |ui| {
                for opt in control_variants().into_iter() {
                    if ui.selectable_label(opt == current, opt.clone()).clicked() {
                        current = opt.clone();
                    }
                }
            })
            .response
            .clone();

        if let Some(parsed) = parse_control(&current) {
            *self = parsed;
        }

        resp
    }
}

fn control_variants() -> Vec<String> {
    let mut v = vec![
        "Mouse::Left".to_string(),
        "Mouse::Middle".to_string(),
        "Mouse::Right".to_string(),
    ];
    let keys = [
        "Escape",
        "Tab",
        "Enter",
        "Space",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
    ];
    for k in keys.iter() {
        v.push(format!("Key::{}", k));
    }
    v
}

fn parse_control(s: &str) -> Option<Control> {
    if s.starts_with("Mouse::") {
        match &s[7..] {
            "Left" => Some(Control::Mouse(winit::event::MouseButton::Left)),
            "Middle" => Some(Control::Mouse(winit::event::MouseButton::Middle)),
            "Right" => Some(Control::Mouse(winit::event::MouseButton::Right)),
            _ => None,
        }
    } else if s.starts_with("Key::") {
        match &s[5..] {
            "Escape" => Some(Control::Key(winit::keyboard::KeyCode::Escape)),
            "Tab" => Some(Control::Key(winit::keyboard::KeyCode::Tab)),
            "Enter" => Some(Control::Key(winit::keyboard::KeyCode::Enter)),
            "Space" => Some(Control::Key(winit::keyboard::KeyCode::Space)),
            "ArrowUp" => Some(Control::Key(winit::keyboard::KeyCode::ArrowUp)),
            "ArrowDown" => Some(Control::Key(winit::keyboard::KeyCode::ArrowDown)),
            "ArrowLeft" => Some(Control::Key(winit::keyboard::KeyCode::ArrowLeft)),
            "ArrowRight" => Some(Control::Key(winit::keyboard::KeyCode::ArrowRight)),
            _ => None,
        }
    } else {
        None
    }
}
