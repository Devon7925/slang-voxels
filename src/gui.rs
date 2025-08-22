use egui::{
    self, Color32, Id, Rect, Rgba, Ui
};

use crate::{
    card_system::{
        Deck, DragableCard,
    }, card_editor::PaletteState, lobby_browser::LobbyBrowser
    // utils::{translate_egui_key_code, translate_egui_pointer_button},
};

const ID_SOURCE: &str = "card_editor";

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum GuiElement {
    EscMenu,
    CardEditor,
    MainMenu,
    MultiplayerMenu,
    LobbyBrowser,
    LobbyQueue,
    SingleplayerMenu,
    ModeGui,
    DeckPicker,
}

pub struct GuiState {
    pub menu_stack: Vec<GuiElement>,
    pub errors: Vec<String>,
    pub gui_deck: Deck,
    pub render_deck: Deck,
    pub render_deck_idx: usize,
    pub dock_cards: Vec<DragableCard>,
    pub cooldown_cache_refresh_delay: f32,
    pub palette_state: PaletteState,
    pub lobby_browser: LobbyBrowser,
    pub should_exit: bool,
    pub game_just_started: bool,
}

// Helper function to center arbitrary widgets. It works by measuring the width of the widgets after rendering, and
// then using that offset on the next frame.
pub fn vertical_centerer(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        let id = ui.id().with("_v_centerer");
    let last_height: Option<f32> = ui.memory_mut(|mem| mem.data.get_temp::<f32>(id));
        if let Some(last_height) = last_height {
            ui.add_space((ui.available_height() - last_height) / 2.0);
        }
        let res = ui
            .scope(|ui| {
                add_contents(ui);
            })
            .response;
        let height = res.rect.height();
        ui.memory_mut(|mem| mem.data.insert_temp(id, height));

        // Repaint if height changed
        match last_height {
            None => ui.ctx().request_repaint(),
            Some(last_height) if last_height != height => ui.ctx().request_repaint(),
            Some(_) => {}
        }
    });
}

pub fn horizontal_centerer(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let id = ui.id().with("_h_centerer");
    let last_width: Option<f32> = ui.memory_mut(|mem| mem.data.get_temp::<f32>(id));
        if let Some(last_width) = last_width {
            ui.add_space((ui.available_width() - last_width) / 2.0);
        }
        let res = ui
            .scope(|ui| {
                add_contents(ui);
            })
            .response;
        let width = res.rect.width();
        ui.memory_mut(|mem| mem.data.insert_temp(id, width));

        // Repaint if height changed
        match last_width {
            None => ui.ctx().request_repaint(),
            Some(last_width) if last_width != width => ui.ctx().request_repaint(),
            Some(_) => {}
        }
    });
}

// pub fn lerp<T>(start: T, end: T, t: f32) -> T
// where
//     T: Add<T, Output = T> + Sub<T, Output = T> + Mul<f32, Output = T> + Copy,
// {
//     (end - start) * t.clamp(0.0, 1.0) + start
// }

// fn get_arc_points(
//     start: f32,
//     center: Pos2,
//     radius: f32,
//     value: f32,
//     max_arc_distance: f32,
// ) -> Vec<Pos2> {
//     let start_turns: f32 = start;
//     let end_turns = start_turns + value;

//     let points = (value.abs() / max_arc_distance).ceil() as usize;
//     let points = points.max(1);
//     (0..=points)
//         .map(|i| {
//             let t = i as f32 / (points - 1) as f32;
//             let angle = lerp(start_turns * TAU, end_turns * TAU, t);
//             let x = radius * angle.cos();
//             let y = -radius * angle.sin();
//             pos2(x, y) + center.to_vec2()
//         })
//         .collect()
// }

// fn get_arc_shape(
//     start: f32,
//     center: Pos2,
//     radius: f32,
//     value: f32,
//     max_arc_distance: f32,
//     stroke: PathStroke,
// ) -> Shape {
//     Shape::Path(PathShape {
//         points: get_arc_points(start, center, radius, value, max_arc_distance),
//         closed: false,
//         fill: Color32::TRANSPARENT,
//         stroke,
//     })
// }

// Helper approximations for modern egui drag checks. The old API exposed
// Memory::is_being_dragged/is_anything_being_dragged which changed; here we
// approximate by checking stored temp data and pointer state. This keeps the
// rest of the GUI logic similar while avoiding private Memory APIs.
fn is_item_being_dragged(ui: &Ui, id: Id) -> bool {
    // We previously stored a temp rect for draggable items each frame. If that
    // temp exists and the pointer is down, treat the item as being dragged.
    let has_prev: bool = ui.data(|d| d.get_temp::<Rect>(id)).is_some();
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    has_prev && pointer_down
}

fn is_anything_being_dragged(ui: &Ui) -> bool {
    // Approximate "anything being dragged" by whether the pointer is down.
    // This is a simplification but matches the UI's needs (visual feedback
    // while a drag-like interaction is happening).
    ui.input(|i| i.pointer.primary_down())
}

// Map egui key to the project's KeyCode. We only need a subset used by the
// UI keybind editor; map common printable keys and fallback to a best-effort
// mapping for others.
fn translate_egui_key_code(key: egui::Key) -> winit::keyboard::KeyCode {
    use egui::Key;
    use winit::keyboard::KeyCode;
    match key {
        Key::Escape => KeyCode::Escape,
        Key::Tab => KeyCode::Tab,
        Key::Enter => KeyCode::Enter,
        Key::Space => KeyCode::Space,
        Key::Backspace => KeyCode::Backspace,
    Key::ArrowDown => KeyCode::ArrowDown,
    Key::ArrowLeft => KeyCode::ArrowLeft,
    Key::ArrowRight => KeyCode::ArrowRight,
    Key::ArrowUp => KeyCode::ArrowUp,
    // For keys we don't explicitly map, return Escape as a harmless
    // default so the function remains total. A better mapping could be
    // added later if needed.
    _ => KeyCode::Escape,
    }
}

fn translate_egui_pointer_button(button: egui::PointerButton) -> winit::event::MouseButton {
    use egui::PointerButton;
    use winit::event::MouseButton;
    match button {
        PointerButton::Primary => MouseButton::Left,
        PointerButton::Secondary => MouseButton::Right,
        PointerButton::Middle => MouseButton::Middle,
        _ => MouseButton::Other(0),
    }
}

// fn cooldown_ui(ui: &mut egui::Ui, ability: &PlayerAbility, ability_idx: usize) -> egui::Response {
//     let desired_size = ui.spacing().interact_size.y * egui::vec2(3.0, 3.0);
//     let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
//     let recovery_bar_rect = rect
//         .with_max_y(rect.min.y + 10.0)
//         .with_max_x(rect.min.x + rect.width() * ability.recovery / ability.value.1[ability_idx]);

//     if ui.is_rect_visible(rect) {
//         let font = egui::FontId::proportional(24.0);
//         if ability.cooldown > 0.0 && ability.remaining_charges == 0 {
//             ui.painter().rect_filled(rect, 5.0, Color32::DARK_GRAY);
//             if ability.recovery > 0.0 {
//                 ui.painter()
//                     .rect_filled(recovery_bar_rect, 5.0, Color32::GREEN);
//             }
//             ui.painter().text(
//                 rect.center(),
//                 Align2::CENTER_CENTER,
//                 format!("{}", ability.cooldown.ceil() as i32),
//                 font.clone(),
//                 Color32::WHITE,
//             );
//             return response;
//         }
//         ui.painter().rect_filled(rect, 5.0, Color32::LIGHT_GRAY);
//         if ability.recovery > 0.0 {
//             ui.painter()
//                 .rect_filled(recovery_bar_rect, 5.0, Color32::GREEN);
//         }
//         {
//             let keybind = &ability.ability.abilities[ability_idx].1;
//             if let Some(key) = keybind.get_simple_representation() {
//                 ui.painter().text(
//                     rect.center(),
//                     Align2::CENTER_CENTER,
//                     format!("{}", key),
//                     font,
//                     Color32::BLACK,
//                 );
//             }
//         }
//         if ability.ability.max_charges > 1 {
//             let font = egui::FontId::proportional(12.0);
//             let to_next_charge = 1.0 - ability.cooldown / ability.value.0;
//             ui.painter()
//                 .circle_filled(rect.right_top(), 8.0, Color32::GRAY);
//             ui.painter().add(get_arc_shape(
//                 0.0,
//                 rect.right_top(),
//                 8.0,
//                 to_next_charge,
//                 0.03,
//                 Stroke::new(1.0, Color32::BLACK),
//             ));
//             ui.painter().text(
//                 rect.right_top(),
//                 Align2::CENTER_CENTER,
//                 format!("{}", ability.remaining_charges),
//                 font,
//                 Color32::BLACK,
//             );
//         }
//     }

//     response
// }

// pub fn cooldown<'a>(ability: &'a PlayerAbility, ability_idx: usize) -> impl egui::Widget + 'a {
//     move |ui: &mut egui::Ui| cooldown_ui(ui, ability, ability_idx)
// }

pub fn darken(color: Color32, factor: f32) -> Color32 {
    let mut color = Rgba::from(color);
    for i in 0..3 {
        color[i] = color[i] * factor;
    }
    color.into()
}

// pub fn healthbar(corner_offset: f32, ctx: &egui::Context, spectate_player: &Entity) {
//     egui::Area::new("healthbar")
//         .anchor(
//             Align2::LEFT_BOTTOM,
//             Vec2::new(corner_offset, -corner_offset),
//         )
//         .show(ctx, |ui| {
//             let thickness = 1.0;
//             let color = Color32::from_additive_luminance(255);
//             let (player_health, player_max_health) = spectate_player.get_health_stats();

//             for AppliedStatusEffect { effect, time_left } in spectate_player.status_effects.iter() {
//                 let effect_name = match effect {
//                     ReferencedStatusEffect::DamageOverTime(stacks) => {
//                         format!("Damage Over Time {}", stacks)
//                     }
//                     ReferencedStatusEffect::Speed(stacks) => format!("Speed {}", stacks),
//                     ReferencedStatusEffect::IncreaseDamageTaken(stacks) => {
//                         format!("Increase Damage Taken {}", stacks)
//                     }
//                     ReferencedStatusEffect::IncreaseGravity(direction, stacks) => {
//                         format!("Increase Gravity {} {}", direction, stacks)
//                     }
//                     ReferencedStatusEffect::Overheal(stacks) => format!("Overheal {}", stacks),
//                     ReferencedStatusEffect::Grow(stacks) => format!("Grow {}", stacks),
//                     ReferencedStatusEffect::IncreaseMaxHealth(stacks) => {
//                         format!("Increase Max Health {}", stacks)
//                     }
//                     ReferencedStatusEffect::Invincibility => "Invincibility".to_string(),
//                     ReferencedStatusEffect::Trapped => "Trapped".to_string(),
//                     ReferencedStatusEffect::Lockout => "Lockout".to_string(),
//                     ReferencedStatusEffect::OnHit(_) => "On Player Hit".to_string(),
//                 };
//                 ui.label(
//                     RichText::new(format!("{}: {:.1}s", effect_name, time_left))
//                         .color(Color32::WHITE),
//                 );
//             }

//             ui.label(
//                 RichText::new(format!("{} / {}", player_health, player_max_health))
//                     .color(Color32::WHITE),
//             );
//             let desired_size = egui::vec2(200.0, 30.0);
//             let (_id, rect) = ui.allocate_space(desired_size);

//             let to_screen =
//                 emath::RectTransform::from_to(Rect::from_x_y_ranges(0.0..=1.0, 0.0..=1.0), rect);

//             let healthbar_size =
//                 Rect::from_min_max(to_screen * pos2(0.0, 0.0), to_screen * pos2(1.0, 1.0));
//             let mut health_rendered = 0.0;
//             for health_section in spectate_player.health.iter() {
//                 let (health_size, health_color) = match health_section {
//                     HealthSection::Health(current, _max) => {
//                         let prev_health_rendered = health_rendered;
//                         health_rendered += current;
//                         (
//                             Rect::from_min_max(
//                                 to_screen * pos2(prev_health_rendered / player_max_health, 0.0),
//                                 to_screen * pos2(health_rendered / player_max_health, 1.0),
//                             ),
//                             Color32::WHITE,
//                         )
//                     }
//                     HealthSection::Overhealth(current, _time) => {
//                         let prev_health_rendered = health_rendered;
//                         health_rendered += current;
//                         (
//                             Rect::from_min_max(
//                                 to_screen * pos2(prev_health_rendered / player_max_health, 0.0),
//                                 to_screen * pos2(health_rendered / player_max_health, 1.0),
//                             ),
//                             Color32::GREEN,
//                         )
//                     }
//                 };
//                 ui.painter().add(epaint::Shape::rect_filled(
//                     health_size,
//                     Rounding::ZERO,
//                     health_color,
//                 ));
//             }

//             ui.painter().rect_stroke(healthbar_size, CornerRadius::ZERO, Stroke::new(thickness, color));
//         });
// }