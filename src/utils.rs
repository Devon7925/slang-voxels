use std::{
    array::from_fn, collections::{HashMap, HashSet, VecDeque}, fs::read_dir, hash::Hash, path::{Path, PathBuf}
};
use winit::keyboard::KeyCode;

pub fn translate_egui_key_code(key: egui::Key) -> KeyCode {
    match key {
        egui::Key::ArrowDown => KeyCode::ArrowDown,
        egui::Key::ArrowLeft => KeyCode::ArrowLeft,
        egui::Key::ArrowRight => KeyCode::ArrowRight,
        egui::Key::ArrowUp => KeyCode::ArrowUp,
        egui::Key::Escape => KeyCode::Escape,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Backspace => KeyCode::Backspace,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Space => KeyCode::Space,
        egui::Key::Insert => KeyCode::Insert,
        egui::Key::Delete => KeyCode::Delete,
        egui::Key::Home => KeyCode::Home,
        egui::Key::End => KeyCode::End,
        egui::Key::PageUp => KeyCode::PageUp,
        egui::Key::PageDown => KeyCode::PageDown,
        egui::Key::Minus => KeyCode::Minus,
        egui::Key::Num0 => KeyCode::Numpad0,
        egui::Key::Num1 => KeyCode::Numpad1,
        egui::Key::Num2 => KeyCode::Numpad2,
        egui::Key::Num3 => KeyCode::Numpad3,
        egui::Key::Num4 => KeyCode::Numpad4,
        egui::Key::Num5 => KeyCode::Numpad5,
        egui::Key::Num6 => KeyCode::Numpad6,
        egui::Key::Num7 => KeyCode::Numpad7,
        egui::Key::Num8 => KeyCode::Numpad8,
        egui::Key::Num9 => KeyCode::Numpad9,
        egui::Key::A => KeyCode::KeyA,
        egui::Key::B => KeyCode::KeyB,
        egui::Key::C => KeyCode::KeyC,
        egui::Key::D => KeyCode::KeyD,
        egui::Key::E => KeyCode::KeyE,
        egui::Key::F => KeyCode::KeyF,
        egui::Key::G => KeyCode::KeyG,
        egui::Key::H => KeyCode::KeyH,
        egui::Key::I => KeyCode::KeyI,
        egui::Key::J => KeyCode::KeyJ,
        egui::Key::K => KeyCode::KeyK,
        egui::Key::L => KeyCode::KeyL,
        egui::Key::M => KeyCode::KeyM,
        egui::Key::N => KeyCode::KeyN,
        egui::Key::O => KeyCode::KeyO,
        egui::Key::P => KeyCode::KeyP,
        egui::Key::Q => KeyCode::KeyQ,
        egui::Key::R => KeyCode::KeyR,
        egui::Key::S => KeyCode::KeyS,
        egui::Key::T => KeyCode::KeyT,
        egui::Key::U => KeyCode::KeyU,
        egui::Key::V => KeyCode::KeyV,
        egui::Key::W => KeyCode::KeyW,
        egui::Key::X => KeyCode::KeyX,
        egui::Key::Y => KeyCode::KeyY,
        egui::Key::Z => KeyCode::KeyZ,
        egui::Key::F1 => KeyCode::F1,
        egui::Key::F2 => KeyCode::F2,
        egui::Key::F3 => KeyCode::F3,
        egui::Key::F4 => KeyCode::F4,
        egui::Key::F5 => KeyCode::F5,
        egui::Key::F6 => KeyCode::F6,
        egui::Key::F7 => KeyCode::F7,
        egui::Key::F8 => KeyCode::F8,
        egui::Key::F9 => KeyCode::F9,
        egui::Key::F10 => KeyCode::F10,
        egui::Key::F11 => KeyCode::F11,
        egui::Key::F12 => KeyCode::F12,
        egui::Key::F13 => KeyCode::F13,
        egui::Key::F14 => KeyCode::F14,
        egui::Key::F15 => KeyCode::F15,
        egui::Key::F16 => KeyCode::F16,
        egui::Key::F17 => KeyCode::F17,
        egui::Key::F18 => KeyCode::F18,
        egui::Key::F19 => KeyCode::F19,
        egui::Key::F20 => KeyCode::F20,
        egui::Key::Copy => KeyCode::Copy,
        egui::Key::Cut => KeyCode::Cut,
        egui::Key::Paste => KeyCode::Paste,
        egui::Key::Colon => todo!(),
        egui::Key::Comma => KeyCode::Comma,
        egui::Key::Backslash => KeyCode::Backslash,
        egui::Key::Slash => KeyCode::Slash,
        egui::Key::Pipe => todo!(),
        egui::Key::Questionmark => todo!(),
        egui::Key::Exclamationmark => todo!(),
        egui::Key::OpenBracket => todo!(),
        egui::Key::CloseBracket => todo!(),
        egui::Key::OpenCurlyBracket => todo!(),
        egui::Key::CloseCurlyBracket => todo!(),
        egui::Key::Backtick => todo!(),
        egui::Key::Period => KeyCode::Period,
        egui::Key::Plus => todo!(),
        egui::Key::Equals => todo!(),
        egui::Key::Semicolon => KeyCode::Semicolon,
        egui::Key::Quote => KeyCode::Quote,
        egui::Key::F21 => KeyCode::F21,
        egui::Key::F22 => KeyCode::F22,
        egui::Key::F23 => KeyCode::F23,
        egui::Key::F24 => KeyCode::F24,
        egui::Key::F25 => KeyCode::F25,
        egui::Key::F26 => KeyCode::F26,
        egui::Key::F27 => KeyCode::F27,
        egui::Key::F28 => KeyCode::F28,
        egui::Key::F29 => KeyCode::F29,
        egui::Key::F30 => KeyCode::F30,
        egui::Key::F31 => KeyCode::F31,
        egui::Key::F32 => KeyCode::F32,
        egui::Key::F33 => KeyCode::F33,
        egui::Key::F34 => KeyCode::F34,
        egui::Key::F35 => KeyCode::F35,
        egui::Key::BrowserBack => KeyCode::BrowserBack,
    }
}

pub fn translate_egui_pointer_button(button: egui::PointerButton) -> winit::event::MouseButton {
    match button {
        egui::PointerButton::Primary => winit::event::MouseButton::Left,
        egui::PointerButton::Secondary => winit::event::MouseButton::Right,
        egui::PointerButton::Middle => winit::event::MouseButton::Middle,
        egui::PointerButton::Extra1 => winit::event::MouseButton::Other(1),
        egui::PointerButton::Extra2 => winit::event::MouseButton::Other(2),
    }
}

pub fn recurse_files(path: impl AsRef<Path>) -> std::io::Result<Vec<PathBuf>> {
    let mut buf = vec![];
    let entries = read_dir(path)?;

    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_dir() {
            let mut subdir = recurse_files(entry.path())?;
            buf.append(&mut subdir);
        }

        if meta.is_file() {
            buf.push(entry.path());
        }
    }

    Ok(buf)
}