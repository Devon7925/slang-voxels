use std::any::Any;

use egui::{
    emath::{self, Numeric}, epaint::{self}, scroll_area::ScrollSource, text::LayoutJob, vec2, Align2, Color32, CornerRadius, CursorIcon, DragValue, FontId, Frame, Id, InnerResponse, Label, LayerId, Layout, Order, Rect, RichText, ScrollArea, Sense, Shape, Stroke, TextFormat, TextStyle, Ui, UiBuilder
};
use itertools::Itertools;

use crate::{card_system::{Ability, BaseCard, Cooldown, CooldownModifier, DirectionCard, DragableCard, Effect, Keybind, MultiCastModifier, PassiveCard, ProjectileModifier, SignedSimpleCooldownModifier, SimpleCooldownModifier, SimpleProjectileModifierType, SimpleStatusEffectType, StatusEffect, UnsignedSimpleStatusEffectType, VoxelMaterial}, gui::{darken, GuiState}, settings_manager::Control, utils::{translate_egui_key_code, translate_egui_pointer_button}};

const ID_SOURCE: &str = "card_editor";

pub enum EditMode {
    FullEditing,
    Readonly,
}
impl EditMode {
    fn can_edit_modifiers(&self) -> bool {
        match self {
            EditMode::FullEditing => true,
            EditMode::Readonly => false,
        }
    }

    fn can_drag_modifiers(&self) -> bool {
        match self {
            EditMode::FullEditing => true,
            EditMode::Readonly => false,
        }
    }

    fn can_drag_base_cards(&self) -> bool {
        match self {
            EditMode::FullEditing => true,
            EditMode::Readonly => false,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum PaletteState {
    ProjectileModifiers,
    BaseCards,
    AdvancedProjectileModifiers,
    MultiCastModifiers,
    CooldownModifiers,
    Materials,
    StatusEffects,
    Directions,
    Dock,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DndBox {
    label: String,
    items: Vec<DndBox>,
}

pub fn dnd_drag_source<Payload, R>(
    ui: &mut Ui,
    id: Id,
    payload: Payload,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>
where
    Payload: Any + Send + Sync,
{
    let is_being_dragged = ui.ctx().is_being_dragged(id);

    if is_being_dragged {
        egui::DragAndDrop::set_payload(ui.ctx(), payload);

        // Paint the body to a new layer:
        let layer_id = LayerId::new(Order::Tooltip, id);
        let InnerResponse { inner, response } =
            ui.scope_builder(UiBuilder::new().layer_id(layer_id), add_contents);

        // Now we move the visuals of the body to where the mouse is.
        // Normally you need to decide a location for a widget first,
        // because otherwise that widget cannot interact with the mouse.
        // However, a dragged component cannot be interacted with anyway
        // (anything with `Order::Tooltip` always gets an empty [`Response`])
        // So this is fine!

        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - response.rect.center();
            ui.ctx()
                .transform_layer_shapes(layer_id, emath::TSTransform::from_translation(delta));
        }

        InnerResponse::new(inner, response)
    } else {
        let dnd_response = ui.data(|d| d.get_temp::<Rect>(id)).map(|prev_frame_rect| {
            ui.interact(prev_frame_rect, id, Sense::drag())
                .on_hover_cursor(CursorIcon::Grab)
        });

        let InnerResponse { inner, response } = ui.scope(add_contents);

        ui.data_mut(|d| d.insert_temp(id, response.rect));

        let response = if let Some(dnd_response) = dnd_response {
            dnd_response | response
        } else {
            response
        };

        InnerResponse::new(inner, response)
    }
}

/// What is being dragged.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Location {
    path: Vec<usize>,
}

#[derive(Debug)]
pub enum DragableType {
    ProjectileModifier,
    MultiCastModifier,
    CooldownModifier,
    StatusEffect,
    BaseCard,
    Direction,
}

#[derive(Debug)]
pub enum DropableType {
    MultiCastBaseCard,
    BaseNone,
    BaseProjectile,
    BaseStatusEffects,
    Cooldown,
    Direction,
    Palette,
}

#[derive(Debug)]
pub enum ModificationType {
    Add,
    Remove,
    Other,
}

pub fn is_valid_drag(from: &DragableType, to: &DropableType) -> bool {
    match (from, to) {
        (DragableType::ProjectileModifier, DropableType::BaseProjectile) => true,
        (DragableType::StatusEffect, DropableType::BaseStatusEffects) => true,
        (DragableType::MultiCastModifier, DropableType::MultiCastBaseCard) => true,
        (DragableType::BaseCard, DropableType::MultiCastBaseCard) => true,
        (DragableType::BaseCard, DropableType::BaseNone) => true,
        (DragableType::CooldownModifier, DropableType::Cooldown) => true,
        (DragableType::BaseCard, DropableType::Cooldown) => true,
        (DragableType::Direction, DropableType::Direction) => true,
        (_, DropableType::Palette) => true,
        _ => false,
    }
}

impl DrawableCard for Cooldown {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let Cooldown {
            abilities,
            modifiers,
            cooldown_value,
        } = self;
        ui.visuals_mut().widgets.active.corner_radius = CornerRadius::from(CARD_UI_ROUNDING);
        ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::from(CARD_UI_ROUNDING);
        ui.visuals_mut().override_text_color = Some(Color32::WHITE);
        ui.visuals_mut().widgets.inactive.bg_stroke =
            Stroke::new(0.5, Color32::from_rgb(255, 0, 255));
        let frame = Frame::default().inner_margin(CARD_UI_SPACING);
        let (response, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(CARD_UI_SPACING);
                    draw_label(
                        ui,
                        "Cooldown",
                        "A set of abilities that can be used once the timer is up".to_string(),
                        modify_path,
                        path,
                    );
                    if let Some(cooldown_value) = cooldown_value {
                        ui.label(format!("{:.2}s", cooldown_value.0))
                            .on_hover_text(format!(
                                "Recoveries: {}",
                                cooldown_value
                                    .1
                                    .iter()
                                    .map(|v| format!("{:.2}s", v))
                                    .join(", ")
                            ));
                    }
                    path.push(0);
                    for (mod_idx, modifier) in modifiers.iter_mut().enumerate() {
                        path.push(mod_idx);
                        modifier.draw(ui, path, dnd_path, modify_path, edit_mode);
                        path.pop();
                    }
                    path.pop();
                    ui.add_space(CARD_UI_SPACING);
                });
                path.push(1);
                for (ability_idx, mut ability) in abilities.iter_mut().enumerate() {
                    path.push(ability_idx);
                    ui.horizontal(|ui| {
                        draw_keybind(ui, &mut ability);
                        ability
                            .card
                            .draw(ui, path, dnd_path, modify_path, edit_mode);
                    });
                    path.pop();
                }
                path.pop();
            });
        });
            if matches!(edit_mode, EditMode::FullEditing) {
                // allocate_ui_at_rect now takes a closure; run the UI code inside
                // that closure so it receives the inner Ui to operate on.
                let inner = ui.allocate_ui_at_rect(
                    response.response.rect.shrink(CARD_UI_SPACING),
                    |x_ui: &mut Ui| {
                        x_ui.with_layout(Layout::right_to_left(egui::Align::Min), |x_ui| {
                            x_ui.visuals_mut().widgets.inactive.bg_stroke =
                                Stroke::new(0.5, Color32::from_rgb(255, 255, 255));
                            if x_ui.button("X").clicked() {
                                *modify_path = Some((path.clone(), ModificationType::Remove));
                            }
                        });
                    },
                );
                let _ = inner; // ignore the response wrapper
            }

        if let Some(drop_result) = payload {
            dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                path: path.clone(),
            }));
        }
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        self.cooldown_value = None;
        let type_idx = path.pop().unwrap() as usize;
        if type_idx == 0 {
            let idx = path.pop().unwrap() as usize;
            self.modifiers[idx].modify_from_path(path, modification_type);
        } else if type_idx == 1 {
            let idx = path.pop().unwrap() as usize;
            self.abilities[idx]
                .card
                .modify_from_path(path, modification_type);
            self.abilities[idx].invalidate_cooldown_cache();
        } else {
            panic!("Invalid state");
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        self.cooldown_value = None;
        let type_idx = path.pop().unwrap() as usize;
        if type_idx == 0 {
            let idx = path.pop().unwrap() as usize;
            self.modifiers[idx].take_from_path(path)
        } else if type_idx == 1 {
            let idx = path.pop().unwrap() as usize;
            if path.is_empty() {
                let ability_card = self.abilities[idx].card.clone();
                self.abilities[idx].card = BaseCard::None;
                self.abilities[idx].invalidate_cooldown_cache();
                DragableCard::BaseCard(ability_card)
            } else {
                let result = self.abilities[idx].card.take_from_path(path);
                self.abilities[idx].invalidate_cooldown_cache();
                result
            }
        } else {
            panic!("Invalid state");
        }
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        self.cooldown_value = None;
        if path.is_empty() {
            if let DragableCard::BaseCard(item) = item {
                self.abilities.push(Ability {
                    card: item,
                    ..Default::default()
                });
            } else if let DragableCard::CooldownModifier(modifier_item) = item {
                let mut combined = false;
                match modifier_item.clone() {
                    CooldownModifier::SimpleCooldownModifier(last_type, last_s) => {
                        for modifier in self.modifiers.iter_mut() {
                            match modifier {
                                CooldownModifier::SimpleCooldownModifier(current_type, s)
                                    if *current_type == last_type =>
                                {
                                    *s += last_s;
                                    combined = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    CooldownModifier::SignedSimpleCooldownModifier(last_type, last_s) => {
                        for modifier in self.modifiers.iter_mut() {
                            match modifier {
                                CooldownModifier::SignedSimpleCooldownModifier(current_type, s)
                                    if *current_type == last_type =>
                                {
                                    *s += last_s;
                                    combined = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }

                if !combined {
                    self.modifiers.push(modifier_item.clone());
                }
            } else {
                panic!("Invalid state")
            }
        } else {
            assert!(path.pop().unwrap() == 1);
            let idx = path.pop().unwrap() as usize;
            self.abilities[idx].card.insert_to_path(path, item);
            self.abilities[idx].invalidate_cooldown_cache();
        }
    }

    fn cleanup(&mut self, path: &mut Vec<usize>) {
        if path.is_empty() {
            return;
        }
        let idx_type = path.pop().unwrap();
        if idx_type == 0 {
            let idx = path.pop().unwrap() as usize;
            assert!(path.is_empty());
            match &mut self.modifiers[idx] {
                CooldownModifier::None => {
                    self.modifiers.remove(idx);
                }
                CooldownModifier::SimpleCooldownModifier(_, s) => {
                    if *s == 0 {
                        self.modifiers.remove(idx);
                    }
                }
                CooldownModifier::SignedSimpleCooldownModifier(_, s) => {
                    if *s == 0 {
                        self.modifiers.remove(idx);
                    }
                }
                _ => {}
            }
        } else if idx_type == 1 {
            let idx = path.pop().unwrap() as usize;
            if path.is_empty() {
                if matches!(self.abilities[idx].card, BaseCard::None) && self.abilities.len() > 1 {
                    self.abilities.remove(idx);
                }
            } else {
                self.abilities[idx].card.cleanup(path);
                self.abilities[idx].invalidate_cooldown_cache();
            }
        } else {
            panic!("Invalid state");
        }
    }
}

fn draw_keybind(ui: &mut Ui, ability: &mut Ability) {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(3.0, 3.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ability.is_keybind_selected {
        ui.ctx().input(|input| {
            for key in input.events.iter() {
                if let egui::Event::Key { key, pressed, .. } = key {
                    if *pressed {
                        ability.keybind =
                            Keybind::Pressed(Control::Key(translate_egui_key_code(*key)));
                        ability.is_keybind_selected = false;
                    }
                }
                if let egui::Event::PointerButton {
                    button, pressed, ..
                } = key
                {
                    if *pressed {
                        ability.keybind = Keybind::Pressed(Control::Mouse(
                            translate_egui_pointer_button(*button),
                        ));
                        ability.is_keybind_selected = false;
                    }
                }
            }
        });
    } else if response.clicked() {
        ability.is_keybind_selected = true;
    }

    if ui.is_rect_visible(rect) {
        let font = egui::FontId::proportional(24.0);

        let fill_color = if ability.is_keybind_selected {
            Color32::DARK_GRAY
        } else {
            Color32::LIGHT_GRAY
        };
        ui.painter().rect_filled(rect, 0.0, fill_color);
        {
            if let Some(key) = ability.keybind.get_simple_representation() {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{}", key),
                    font,
                    Color32::BLACK,
                );
            }
        }
    }
}

const CARD_UI_SPACING: f32 = 3.0;
const CARD_UI_ROUNDING: f32 = 3.0;
trait DrawableCard {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    );

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType);
    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard;
    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard);
    fn cleanup(&mut self, path: &mut Vec<usize>);
}

impl DrawableCard for CooldownModifier {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        _dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let hover_text = self.get_hover_text();
        let name = self.get_name();
        match self {
            CooldownModifier::SimpleCooldownModifier(_, v) => draw_modifier(
                ui,
                item_id,
                name,
                Some(v),
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            CooldownModifier::SignedSimpleCooldownModifier(_, v) => draw_modifier(
                ui,
                item_id,
                name,
                Some(v),
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            CooldownModifier::None | CooldownModifier::Reloading => draw_modifier(
                ui,
                item_id,
                name,
                None::<&mut u32>,
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
        }
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        assert!(path.is_empty());
        match self {
            CooldownModifier::SimpleCooldownModifier(_, v) => match modification_type {
                ModificationType::Add => *v += 1,
                ModificationType::Remove => {
                    if *v > 1 {
                        *v -= 1
                    }
                }
                ModificationType::Other => {}
            },
            CooldownModifier::SignedSimpleCooldownModifier(_, v) => match modification_type {
                ModificationType::Add => *v += 1,
                ModificationType::Remove => *v -= 1,
                ModificationType::Other => {}
            },
            _ => {}
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        assert!(path.is_empty());
        let modifier = self.clone();
        *self = CooldownModifier::None;
        DragableCard::CooldownModifier(modifier)
    }

    fn insert_to_path(&mut self, _path: &mut Vec<usize>, _item: DragableCard) {}

    fn cleanup(&mut self, _path: &mut Vec<usize>) {}
}

impl DrawableCard for PassiveCard {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        ui.vertical(|ui| {
            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(0.5, Color32::KHAKI);
            ui.visuals_mut().widgets.inactive.bg_fill =
                darken(ui.visuals_mut().widgets.inactive.bg_stroke.color, 0.25);

            let frame = Frame::default().inner_margin(CARD_UI_SPACING);
            let (response, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                let mut advanced_effects = vec![];
                ui.horizontal(|ui| {
                    ui.add_space(CARD_UI_SPACING);
                    ui.vertical(|ui| {
                        ui.add_space(CARD_UI_SPACING);
                        ui.horizontal_wrapped(|ui| {
                            draw_label(
                                ui,
                                "Passive Status Effects",
                                format!("Apply status effects passively"),
                                modify_path,
                                path,
                            );
                            for (effect_idx, effect) in self.passive_effects.iter_mut().enumerate()
                            {
                                if effect.is_advanced() {
                                    advanced_effects.push((effect_idx, effect));
                                    continue;
                                }
                                path.push(effect_idx);
                                effect.draw(
                                    ui,
                                    path,
                                    dnd_path,
                                    modify_path,
                                    edit_mode,
                                );
                                path.pop();
                            }
                        });

                        for (modifier_idx, modifier) in advanced_effects.into_iter() {
                            path.push(modifier_idx);
                            modifier.draw(ui, path, dnd_path, modify_path, edit_mode);
                            path.pop();
                        }
                        ui.add_space(CARD_UI_SPACING);
                    });
                    ui.add_space(CARD_UI_SPACING);
                });
            });

            if let Some(drop_result) = payload {
                dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                    path: path.clone(),
                }));
            }
        });
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        if path.is_empty() {
        } else {
            let effect_idx = path.pop().unwrap() as usize;
            self.passive_effects[effect_idx].modify_from_path(path, modification_type);
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        let Some(effect_idx) = path.pop() else {
            panic!("Invalid state: path is empty");
        };
        if path.is_empty() {
            let effect = self.passive_effects[effect_idx as usize].clone();
            self.passive_effects[effect_idx as usize] = StatusEffect::None;
            return DragableCard::StatusEffect(effect);
        }
        self.passive_effects
            .get_mut(effect_idx as usize)
            .unwrap()
            .take_from_path(path)
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        if path.is_empty() {
            let DragableCard::StatusEffect(item) = item else {
                panic!("Invalid state")
            };
            if let StatusEffect::SimpleStatusEffect(last_ty, last_s) = item.clone() {
                let mut combined = false;
                for effect in self.passive_effects.iter_mut() {
                    match effect {
                        StatusEffect::SimpleStatusEffect(ty, s) => {
                            if last_ty == *ty {
                                *s += last_s;
                                combined = true;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if !combined {
                    self.passive_effects.push(item.clone());
                }
            } else {
                self.passive_effects.push(item.clone());
            }

            self.passive_effects.retain(|effect| match effect {
                StatusEffect::SimpleStatusEffect(_, s) => *s != 0,
                _ => true,
            });
        } else {
            let idx = path.pop().unwrap() as usize;
            assert!(path.pop().unwrap() == 0);
            self.passive_effects[idx].insert_to_path(path, item);
        }
    }

    fn cleanup(&mut self, path: &mut Vec<usize>) {
        if path.len() <= 1 {
            self.passive_effects.retain(|effect| match effect {
                StatusEffect::None => false,
                _ => true,
            });
        } else {
            let idx = path.pop().unwrap() as usize;
            assert!(path.pop().unwrap() == 0);
            match &mut self.passive_effects[idx] {
                StatusEffect::OnHit(card_box) => {
                    card_box.cleanup(path)
                }
                StatusEffect::SimpleStatusEffect(SimpleStatusEffectType::IncreaseGravity(_), _) => {
                }
                ref invalid => panic!(
                    "Invalid state: cannot follow path {} into {:?}",
                    idx, invalid
                ),
            }
        }
    }
}

impl DrawableCard for MultiCastModifier {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        _dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let hover_text = self.get_hover_text();
        let name = self.get_name();
        match self {
            MultiCastModifier::None => draw_label(ui, &name, hover_text, modify_path, path),
            MultiCastModifier::Spread(v) | MultiCastModifier::Duplication(v) => {
                draw_modifier(
                    ui,
                    item_id,
                    name,
                    Some(v),
                    hover_text,
                    true,
                    modify_path,
                    path,
                    edit_mode,
                )
            }
        }
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        assert!(path.is_empty());
        match self {
            MultiCastModifier::None => {}
            MultiCastModifier::Spread(value) => match modification_type {
                ModificationType::Add => *value += 1,
                ModificationType::Remove => {
                    if *value > 1 {
                        *value -= 1
                    }
                }
                ModificationType::Other => {}
            },
            MultiCastModifier::Duplication(value) => match modification_type {
                ModificationType::Add => *value += 1,
                ModificationType::Remove => {
                    if *value > 1 {
                        *value -= 1
                    }
                }
                ModificationType::Other => {}
            },
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        assert!(path.is_empty());
        let multicast_modifier = self.clone();
        *self = MultiCastModifier::None;
        DragableCard::MultiCastModifier(multicast_modifier)
    }

    fn insert_to_path(&mut self, _path: &mut Vec<usize>, _item: DragableCard) {}

    fn cleanup(&mut self, _path: &mut Vec<usize>) {}
}

impl DrawableCard for ProjectileModifier {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let mut advanced_modifier = false;
        let hover_text = self.get_hover_text();
        let name = self.get_name();
        match self {
            ProjectileModifier::SimpleModify(_, v) => draw_modifier(
                ui,
                item_id,
                name,
                Some(v),
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),

            ProjectileModifier::LockToOwner(direction) => {
                draw_modifier(
                    ui,
                    item_id,
                    name,
                    None::<&mut u32>,
                    hover_text,
                    true,
                    modify_path,
                    path,
                    edit_mode,
                );
                path.push(0);
                direction.draw(ui, path, dnd_path, modify_path, edit_mode);
                path.pop();
            }
            ProjectileModifier::NoEnemyFire
            | ProjectileModifier::FriendlyFire
            | ProjectileModifier::PiercePlayers
            | ProjectileModifier::WallBounce
            | ProjectileModifier::None => draw_modifier(
                ui,
                item_id,
                name,
                None::<&mut u32>,
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            ref modifier if modifier.is_advanced() => {
                advanced_modifier = true;
            }
            _ => panic!("Invalid State"),
        }
        if advanced_modifier {
            let hover_text = self.get_hover_text();
            let name = self.get_name();
            ui.horizontal(|ui| {
                ui.add_space(CARD_UI_SPACING);
                match self {
                    ProjectileModifier::OnHit(base_card)
                    | ProjectileModifier::OnHeadshot(base_card)
                    | ProjectileModifier::OnExpiry(base_card) => {
                        dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| {
                            draw_modifier(
                                ui,
                                item_id,
                                name.clone(),
                                None::<&mut u32>,
                                hover_text.clone(),
                                false,
                                modify_path,
                                path,
                                edit_mode,
                            );
                            path.push(0);
                            base_card.draw(
                                ui,
                                path,
                                dnd_path,
                                modify_path,
                                edit_mode,
                            );
                            path.pop();
                        });
                    }
                    ProjectileModifier::OnTrigger(frequency, base_card)
                    | ProjectileModifier::Trail(frequency, base_card) => {
                        dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| {
                            draw_modifier(
                                ui,
                                item_id,
                                name.clone(),
                                Some(frequency),
                                hover_text.clone(),
                                false,
                                modify_path,
                                path,
                                edit_mode,
                            );
                            path.push(0);
                            base_card.draw(
                                ui,
                                path,
                                dnd_path,
                                modify_path,
                                edit_mode,
                            );
                            path.pop();
                        });
                    }
                    _ => panic!("Invalid State"),
                }
            });
        }
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        if path.is_empty() {
            match self {
                ProjectileModifier::SimpleModify(_type, value) => match modification_type {
                    ModificationType::Add => *value += 1,
                    ModificationType::Remove => *value -= 1,
                    ModificationType::Other => {}
                },
                ProjectileModifier::Trail(frequency, _card) => match modification_type {
                    ModificationType::Add => *frequency += 1,
                    ModificationType::Remove => {
                        if *frequency > 1 {
                            *frequency -= 1
                        }
                    }
                    ModificationType::Other => {}
                },
                ProjectileModifier::OnTrigger(id, _card) => match modification_type {
                    ModificationType::Add => *id += 1,
                    ModificationType::Remove => {
                        if *id > 0 {
                            *id -= 1
                        }
                    }
                    ModificationType::Other => {}
                },
                ProjectileModifier::FriendlyFire
                | ProjectileModifier::LockToOwner(_)
                | ProjectileModifier::NoEnemyFire
                | ProjectileModifier::PiercePlayers
                | ProjectileModifier::OnHeadshot(_)
                | ProjectileModifier::OnHit(_)
                | ProjectileModifier::OnExpiry(_)
                | ProjectileModifier::WallBounce
                | ProjectileModifier::None => {}
            }
        } else {
            assert!(path.pop().unwrap() == 0);
            match self {
                ProjectileModifier::OnHit(card) => {
                    card.modify_from_path(path, modification_type)
                }
                ProjectileModifier::OnHeadshot(card) => {
                    card.modify_from_path(path, modification_type)
                }
                ProjectileModifier::OnExpiry(card) => {
                    card.modify_from_path(path, modification_type)
                }
                ProjectileModifier::OnTrigger(_, card) => {
                    card.modify_from_path(path, modification_type)
                }
                ProjectileModifier::Trail(_freqency, card) => {
                    card.modify_from_path(path, modification_type)
                }
                _ => panic!("Invalid state"),
            }
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        if path.is_empty() {
            let value = self.clone();
            *self = ProjectileModifier::None;
            DragableCard::ProjectileModifier(value)
        } else {
            assert!(path.pop().unwrap() == 0);
            if path.is_empty() {
                if let ProjectileModifier::LockToOwner(direction) = self {
                    return direction.take_from_path(path);
                }
                let card_ref = match self {
                    ProjectileModifier::OnHit(card)
                    | ProjectileModifier::OnHeadshot(card)
                    | ProjectileModifier::OnExpiry(card)
                    | ProjectileModifier::OnTrigger(_, card)
                    | ProjectileModifier::Trail(_, card) => card,
                    invalid_take_modifier => panic!(
                        "Invalid state: cannot take from {:?}",
                        invalid_take_modifier
                    ),
                };
                let result = DragableCard::BaseCard(card_ref.clone());
                *card_ref = BaseCard::None;
                result
            } else {
                match self {
                    ProjectileModifier::OnHit(card)
                    | ProjectileModifier::OnHeadshot(card)
                    | ProjectileModifier::OnExpiry(card)
                    | ProjectileModifier::OnTrigger(_, card)
                    | ProjectileModifier::Trail(_, card) => card.take_from_path(path),
                    _ => panic!("Invalid state"),
                }
            }
        }
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        assert!(path.pop().unwrap() == 0);
        match self {
            ProjectileModifier::OnHit(card)
            | ProjectileModifier::OnHeadshot(card)
            | ProjectileModifier::OnExpiry(card)
            | ProjectileModifier::OnTrigger(_, card)
            | ProjectileModifier::Trail(_, card) => card.insert_to_path(path, item),
            ProjectileModifier::LockToOwner(direction) => direction.insert_to_path(path, item),
            _ => panic!("Invalid state"),
        }
    }

    fn cleanup(&mut self, path: &mut Vec<usize>) {
        assert!(path.pop().unwrap() == 0);
        match self {
            ProjectileModifier::OnHit(card)
            | ProjectileModifier::OnHeadshot(card)
            | ProjectileModifier::OnExpiry(card)
            | ProjectileModifier::OnTrigger(_, card)
            | ProjectileModifier::Trail(_, card) => card.cleanup(path),
            ProjectileModifier::LockToOwner(direction) => direction.cleanup(path),
            ref invalid => panic!(
                "Invalid state: cannot follow path {:?} into {:?}",
                path, invalid
            ),
        }
    }
}

impl DrawableCard for StatusEffect {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let mut advanced_effect = false;
        let name = self.get_name();
        let hover_text = self.get_hover_text();
        match self {
            StatusEffect::SimpleStatusEffect(
                SimpleStatusEffectType::IncreaseGravity(direction),
                v,
            ) => {
                dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| {
                    draw_modifier(
                        ui,
                        item_id,
                        name.clone(),
                        Some(v),
                        hover_text.clone(),
                        false,
                        modify_path,
                        path,
                        edit_mode,
                    );
                    path.push(0);
                    direction.draw(ui, path, dnd_path, modify_path, edit_mode);
                    path.pop();
                });
            }
            StatusEffect::SimpleStatusEffect(_, v) => draw_modifier(
                ui,
                item_id,
                name,
                Some(v),
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            StatusEffect::UnsignedSimpleStatusEffect(_, v) => draw_modifier(
                ui,
                item_id,
                name,
                Some(v),
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            StatusEffect::Invincibility
            | StatusEffect::Lockout
            | StatusEffect::Trapped
            | StatusEffect::Stun
            | StatusEffect::None => draw_modifier(
                ui,
                item_id,
                name,
                None::<&mut u32>,
                hover_text,
                true,
                modify_path,
                path,
                edit_mode,
            ),
            ref modifier if modifier.is_advanced() => {
                advanced_effect = true;
            }
            _ => panic!("Invalid State"),
        }
        if advanced_effect {
            ui.horizontal(|ui| {
                ui.add_space(CARD_UI_SPACING);
                let hover_text = self.get_hover_text();
                match self {
                    StatusEffect::OnHit(base_card) => {
                        dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| {
                            draw_label(ui, "On Hit", hover_text.clone(), modify_path, path);
                            path.push(0);
                            base_card.draw(
                                ui,
                                path,
                                dnd_path,
                                modify_path,
                                edit_mode,
                            );
                            path.pop();
                        });
                    }
                    _ => panic!("Invalid State"),
                }
            });
        }
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        match self {
            StatusEffect::SimpleStatusEffect(_, stacks) => match modification_type {
                ModificationType::Add => *stacks += 1,
                ModificationType::Remove => *stacks -= 1,
                ModificationType::Other => {}
            },
            StatusEffect::UnsignedSimpleStatusEffect(_, stacks) => {
                match modification_type {
                    ModificationType::Add => *stacks += 1,
                    ModificationType::Remove => {
                        if *stacks > 0 {
                            *stacks -= 1
                        }
                    }
                    ModificationType::Other => {}
                }
            }
            StatusEffect::None
            | StatusEffect::Invincibility
            | StatusEffect::Trapped
            | StatusEffect::Lockout
            | StatusEffect::Stun => {}
            StatusEffect::OnHit(card) => {
                assert!(path.pop().unwrap() == 0);
                card.modify_from_path(path, modification_type)
            }
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        match self {
            StatusEffect::OnHit(card) => {
                assert!(path.pop().unwrap() == 0);
                card.take_from_path(path)
            }
            StatusEffect::SimpleStatusEffect(
                SimpleStatusEffectType::IncreaseGravity(direction),
                _,
            ) => {
                assert!(path.pop().unwrap() == 0);
                direction.take_from_path(path)
            }
            _ => panic!("Invalid state"),
        }
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        match self {
            StatusEffect::OnHit(card) => card.insert_to_path(path, item),
            StatusEffect::SimpleStatusEffect(
                SimpleStatusEffectType::IncreaseGravity(direction),
                _,
            ) => {
                direction.insert_to_path(path, item);
            }
            _ => panic!("Invalid state"),
        }
    }

    fn cleanup(&mut self, path: &mut Vec<usize>) {
        assert!(path.pop().unwrap() == 0);
        match self {
            StatusEffect::OnHit(card) => card.cleanup(path),
            StatusEffect::SimpleStatusEffect(
                SimpleStatusEffectType::IncreaseGravity(direction),
                _,
            ) => {
                direction.cleanup(path);
            }
            _ => {}
        }
    }
}

impl DrawableCard for DirectionCard {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let is_draggable = !matches!(self, DirectionCard::None) && edit_mode.can_drag_modifiers();
        dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| {
            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(0.5, Color32::TRANSPARENT);
            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
            let frame = Frame::default().inner_margin(CARD_UI_SPACING);
            let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                let where_to_put_background = ui.painter().add(Shape::Noop);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let name = match self {
                            DirectionCard::None => "None",
                            DirectionCard::Up => "Up",
                            DirectionCard::Forward => "Forward",
                            DirectionCard::Movement => "Movement",
                        };
                        draw_label(ui, name, name.to_string(), modify_path, path);
                        ui.add_space(CARD_UI_SPACING);
                    });
                });

                let color = Color32::from_rgb(100, 255, 150);
                let min_rect = ui.min_rect().expand(CARD_UI_SPACING / 2.0);
                ui.painter().set(
                    where_to_put_background,
                    epaint::PathShape::convex_polygon(
                        vec![
                            min_rect.left_bottom(),
                            min_rect.left_top(),
                            min_rect.right_top() - vec2(10.0, 0.0),
                            min_rect.right_top().lerp(min_rect.right_bottom(), 0.5),
                            min_rect.right_bottom() - vec2(10.0, 0.0),
                        ],
                        darken(color, 0.25),
                        Stroke::new(1.0, color),
                    ),
                );
            });

            if let Some(drop_result) = payload {
                dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                    path: path.clone(),
                }));
            }
        });
    }

    fn modify_from_path(
        &mut self,
        _path: &mut Vec<usize>,
        _modification_type: ModificationType,
    ) {
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        assert!(path.is_empty());
        let taken_direction = self.clone();
        *self = DirectionCard::None;
        DragableCard::Direction(taken_direction)
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        assert!(path.is_empty());
        if let DragableCard::Direction(new_direction) = item {
            *self = new_direction;
        } else {
            panic!("Invalid state");
        }
    }

    fn cleanup(&mut self, _path: &mut Vec<usize>) {}
}

impl DrawableCard for BaseCard {
    fn draw(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        let item_id = egui::Id::new(ID_SOURCE).with(path.clone());
        let is_draggable = !matches!(self, BaseCard::None | BaseCard::Palette(_))
            && edit_mode.can_drag_base_cards();
        dnd_drag_source::<Location, _>(ui,item_id, Location { path: path.clone() }, |ui| match self {
            BaseCard::Projectile(modifiers) => {
                ui.vertical(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(0.5, Color32::WHITE);
                    ui.visuals_mut().widgets.inactive.bg_fill =
                        darken(ui.visuals_mut().widgets.inactive.bg_stroke.color, 0.25);

                    let frame = Frame::default().inner_margin(CARD_UI_SPACING);
                    let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                        let mut advanced_modifiers = vec![];
                        ui.horizontal(|ui| {
                            ui.add_space(CARD_UI_SPACING);
                            ui.vertical(|ui| {
                                ui.add_space(CARD_UI_SPACING);
                                ui.horizontal_wrapped(|ui| {
                                    ui.add(Label::new("Create Projectile").selectable(false));
                                    for (modifier_idx, modifier) in modifiers.iter_mut().enumerate()
                                    {
                                        if modifier.is_advanced() {
                                            advanced_modifiers.push((modifier_idx, modifier));
                                            continue;
                                        }
                                        path.push(modifier_idx);
                                        modifier.draw(
                                            ui,
                                            path,
                                            dnd_path,
                                            modify_path,
                                            edit_mode,
                                        );
                                        path.pop();
                                    }
                                });

                                for (modifier_idx, modifier) in advanced_modifiers.into_iter() {
                                    path.push(modifier_idx);
                                    modifier.draw(
                                        ui,
                                        path,
                                        dnd_path,
                                        modify_path,
                                        edit_mode,
                                    );
                                    path.pop();
                                }
                                ui.add_space(CARD_UI_SPACING);
                            });
                            ui.add_space(CARD_UI_SPACING);
                        });
                    });

                    if let Some(drop_result) = payload {
                        dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                            path: path.clone(),
                        }));
                    }
                });
            }
            BaseCard::MultiCast(cards, modifiers) => {
                ui.vertical(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(0.5, Color32::YELLOW);
                    ui.visuals_mut().widgets.inactive.bg_fill =
                        darken(ui.visuals_mut().widgets.inactive.bg_stroke.color, 0.25);
                    let frame = Frame::default().inner_margin(CARD_UI_SPACING);
                    let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(CARD_UI_SPACING);
                            ui.vertical(|ui| {
                                ui.add_space(CARD_UI_SPACING);
                                ui.horizontal_wrapped(|ui| {
                                    ui.add(Label::new("Multicast").selectable(false));
                                    path.push(0);
                                    for (mod_idx, modifier) in modifiers.iter_mut().enumerate() {
                                        path.push(mod_idx);
                                        modifier.draw(
                                            ui,
                                            path,
                                            dnd_path,
                                            modify_path,
                                            edit_mode,
                                        );
                                        path.pop();
                                    }
                                    path.pop();
                                });
                                path.push(1);
                                for (card_idx, card) in cards.iter_mut().enumerate() {
                                    path.push(card_idx);
                                    card.draw(
                                        ui,
                                        path,
                                        dnd_path,
                                        modify_path,
                                        edit_mode,
                                    );
                                    path.pop();
                                }
                                path.pop();
                                ui.add_space(CARD_UI_SPACING);
                            });
                            ui.add_space(CARD_UI_SPACING);
                        });
                    });

                    if let Some(drop_result) = payload {
                        dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                            path: path.clone(),
                        }));
                    }
                });
            }
            BaseCard::CreateMaterial(mat) => {
                let where_to_put_background = ui.painter().add(Shape::Noop);
                ui.vertical(|ui| {
                    ui.add_space(CARD_UI_SPACING);
                    ui.horizontal(|ui| {
                        ui.add_space(CARD_UI_SPACING);
                        ui.add(Label::new("Create Material").selectable(false));
                        ui.add(Label::new(format!("{:?}", mat)).selectable(false));
                        ui.add_space(CARD_UI_SPACING);
                    });
                    ui.add_space(CARD_UI_SPACING);
                });

                let color = Color32::BLUE;
                ui.painter().set(
                    where_to_put_background,
                    epaint::RectShape::filled(ui.min_rect(), CornerRadius::from(CARD_UI_ROUNDING), darken(color, 0.25)),
                );
                ui.painter().add(epaint::PathShape::convex_polygon(
                    vec![
                        ui.min_rect().left_top(),
                        ui.min_rect().right_top(),
                        ui.min_rect().right_bottom(),
                        ui.min_rect().left_bottom(),
                    ],
                    Color32::TRANSPARENT,
                    Stroke::new(1.0, color),
                ));
            }
            BaseCard::Effect(effect) => {
                let where_to_put_background = ui.painter().add(Shape::Noop);
                let name = effect.get_name();
                let hover_text = effect.get_hover_text();
                ui.vertical(|ui| {
                    ui.add_space(CARD_UI_SPACING);
                    ui.horizontal(|ui| {
                        ui.add_space(CARD_UI_SPACING);
                        
                        ui.add(Label::new("Apply Effect").selectable(false));
                        match effect {
                            Effect::Damage(v) => draw_modifier(
                                ui,
                                item_id,
                                name,
                                Some(v),
                                hover_text,
                                false,
                                modify_path,
                                path,
                                edit_mode,
                            ),
                            Effect::Knockback(v, direction) => {
                                draw_modifier(
                                    ui,
                                    item_id,
                                    name,
                                    Some(v),
                                    hover_text,
                                    false,
                                    modify_path,
                                    path,
                                    edit_mode,
                                );
                                path.push(0);
                                direction.draw(
                                    ui,
                                    path,
                                    dnd_path,
                                    modify_path,
                                    edit_mode,
                                );
                                path.pop();
                            }
                            Effect::Cleanse | Effect::Teleport => draw_modifier(
                                ui,
                                item_id,
                                name,
                                None::<&mut u32>,
                                hover_text,
                                false,
                                modify_path,
                                path,
                                edit_mode,
                            ),
                        }
                        ui.add_space(CARD_UI_SPACING);
                    });
                    ui.add_space(CARD_UI_SPACING);
                });

                let color = Color32::RED;
                ui.painter().set(
                    where_to_put_background,
                    epaint::RectShape::filled(ui.min_rect(), CornerRadius::from(CARD_UI_ROUNDING), darken(color, 0.25)),
                );
                ui.painter().add(epaint::PathShape::convex_polygon(
                    vec![
                        ui.min_rect().left_top(),
                        ui.min_rect().right_top(),
                        ui.min_rect().right_bottom(),
                        ui.min_rect().left_bottom(),
                    ],
                    Color32::TRANSPARENT,
                    Stroke::new(1.0, color),
                ));
            }
            BaseCard::StatusEffects(duration, effects) => {
                let hover_text = format!(
                    "Apply status effects for a duration of {}s",
                    *duration as f32 * BaseCard::EFFECT_LENGTH_SCALE
                );
                ui.vertical(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        Stroke::new(0.5, Color32::LIGHT_RED);
                    ui.visuals_mut().widgets.inactive.bg_fill =
                        darken(ui.visuals_mut().widgets.inactive.bg_stroke.color, 0.25);

                    let frame = Frame::default().inner_margin(CARD_UI_SPACING);
                    let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                        let mut advanced_effects = vec![];
                        ui.horizontal(|ui| {
                            ui.add_space(CARD_UI_SPACING);
                            ui.vertical(|ui| {
                                ui.add_space(CARD_UI_SPACING);
                                ui.horizontal_wrapped(|ui| {
                                    draw_modifier(
                                        ui,
                                        item_id,
                                        "Apply Status Effects".to_string(),
                                        Some(duration),
                                        hover_text,
                                        false,
                                        modify_path,
                                        path,
                                        edit_mode,
                                    );
                                    for (effect_idx, effect) in effects.iter_mut().enumerate() {
                                        if effect.is_advanced() {
                                            advanced_effects.push((effect_idx, effect));
                                            continue;
                                        }
                                        path.push(effect_idx);
                                        effect.draw(
                                            ui,
                                            path,
                                            dnd_path,
                                            modify_path,
                                            edit_mode,
                                        );
                                        path.pop();
                                    }
                                });

                                for (modifier_idx, modifier) in advanced_effects.into_iter() {
                                    path.push(modifier_idx);
                                    modifier.draw(
                                        ui,
                                        path,
                                        dnd_path,
                                        modify_path,
                                        edit_mode,
                                    );
                                    path.pop();
                                }
                                ui.add_space(CARD_UI_SPACING);
                            });
                            ui.add_space(CARD_UI_SPACING);
                        });
                    });

                    if let Some(drop_result) = payload {
                        dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                            path: path.clone(),
                        }));
                    }
                });
            }
            BaseCard::Trigger(id) => {
                let where_to_put_background = ui.painter().add(Shape::Noop);
                ui.vertical(|ui| {
                    ui.add_space(CARD_UI_SPACING);
                    ui.horizontal(|ui| {
                        ui.add_space(CARD_UI_SPACING);
                        draw_modifier(
                            ui,
                            item_id,
                            "Trigger".to_string(),
                            Some(id),
                            "Cause on trigger events to activate".to_string(),
                            false,
                            modify_path,
                            path,
                            edit_mode,
                        );
                        ui.add_space(CARD_UI_SPACING);
                    });
                    ui.add_space(CARD_UI_SPACING);
                });

                let color = Color32::from_rgb(0, 255, 255);
                ui.painter().set(
                    where_to_put_background,
                    epaint::RectShape::filled(ui.min_rect(), CornerRadius::from(CARD_UI_ROUNDING), darken(color, 0.25)),
                );
                ui.painter().add(epaint::PathShape::convex_polygon(
                    vec![
                        ui.min_rect().left_top(),
                        ui.min_rect().right_top(),
                        ui.min_rect().right_bottom(),
                        ui.min_rect().left_bottom(),
                    ],
                    Color32::TRANSPARENT,
                    Stroke::new(1.0, color),
                ));
            }
            BaseCard::None => {
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::GREEN);
                ui.visuals_mut().widgets.inactive.bg_fill =
                    darken(ui.visuals_mut().widgets.inactive.bg_stroke.color, 0.25);
                let frame = Frame::default().inner_margin(CARD_UI_SPACING);
                let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(CARD_UI_SPACING);
                        ui.add(Label::new("None").selectable(false));
                        ui.add_space(CARD_UI_SPACING);
                    });
                    ui.add_space(CARD_UI_SPACING);
                });

                if let Some(drop_result) = payload {
                    dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                        path: path.clone(),
                    }));
                }
            }
            BaseCard::Palette(palette_cards) => {
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::GRAY);
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::BLACK;
                let frame = Frame::default().inner_margin(CARD_UI_SPACING);
                let (_, payload) = ui.dnd_drop_zone::<Location, _>(frame, |ui| {
                    ui.set_min_size(vec2(200.0, 40.0));
                    ui.add_space(CARD_UI_SPACING);
                    ui.horizontal_wrapped(|ui| {
                        ui.set_row_height(ui.spacing().interact_size.y);
                        for (card_idx, card) in palette_cards.iter_mut().enumerate() {
                            path.push(card_idx);
                            card.draw_draggable(
                                ui,
                                path,
                                dnd_path,
                                modify_path,
                                edit_mode,
                            );
                            path.pop();
                        }
                    });
                    ui.add_space(CARD_UI_SPACING);
                });

                if let Some(drop_result) = payload {
                    dnd_path.get_or_insert((drop_result.as_ref().clone(), Location {
                        path: path.clone(),
                    }));
                }
            }
        });
    }

    fn modify_from_path(&mut self, path: &mut Vec<usize>, modification_type: ModificationType) {
        match self {
            BaseCard::Projectile(modifiers) => {
                let idx = path.pop().unwrap() as usize;
                modifiers[idx].modify_from_path(path, modification_type);
            }
            BaseCard::MultiCast(cards, modifiers) => {
                let type_idx = path.pop().unwrap() as usize;
                if type_idx == 0 {
                    let idx = path.pop().unwrap() as usize;
                    modifiers[idx].modify_from_path(path, modification_type);
                } else if type_idx == 1 {
                    let idx = path.pop().unwrap() as usize;
                    cards[idx].modify_from_path(path, modification_type)
                } else {
                    panic!("Invalid state");
                }
            }
            BaseCard::CreateMaterial(_) => panic!("Invalid state"),
            BaseCard::StatusEffects(duration, effects) => {
                if path.is_empty() {
                    match modification_type {
                        ModificationType::Add => *duration += 1,
                        ModificationType::Remove => {
                            if *duration > 1 {
                                *duration -= 1
                            }
                        }
                        ModificationType::Other => {}
                    }
                } else {
                    let effect_idx = path.pop().unwrap() as usize;
                    effects[effect_idx].modify_from_path(path, modification_type);
                }
            }
            BaseCard::Effect(effect) => {
                assert!(path.is_empty());
                match effect {
                    Effect::Damage(damage) => match modification_type {
                        ModificationType::Add => *damage += 1,
                        ModificationType::Remove => *damage -= 1,
                        ModificationType::Other => {}
                    },
                    Effect::Knockback(knockback, _) => match modification_type {
                        ModificationType::Add => *knockback += 1,
                        ModificationType::Remove => *knockback -= 1,
                        ModificationType::Other => {}
                    },
                    Effect::Cleanse => {}
                    Effect::Teleport => {}
                }
            }
            BaseCard::Trigger(id) => match modification_type {
                ModificationType::Add => *id += 1,
                ModificationType::Remove => {
                    if *id > 0 {
                        *id -= 1
                    }
                }
                ModificationType::Other => {}
            },
            BaseCard::None => panic!("Invalid state"),
            BaseCard::Palette(..) => {}
        }
    }

    fn take_from_path(&mut self, path: &mut Vec<usize>) -> DragableCard {
        if path.is_empty() {
            let result = DragableCard::BaseCard(self.clone());
            *self = BaseCard::None;
            return result;
        }
        match self {
            BaseCard::Projectile(modifiers) => {
                let idx = path.pop().unwrap() as usize;
                modifiers[idx].take_from_path(path)
            }
            BaseCard::MultiCast(cards, modifiers) => {
                let type_idx = path.pop().unwrap() as usize;
                if type_idx == 0 {
                    let idx = path.pop().unwrap() as usize;
                    modifiers[idx].take_from_path(path)
                } else if type_idx == 1 {
                    let idx = path.pop().unwrap() as usize;
                    if path.is_empty() {
                        let value = cards[idx].clone();
                        cards[idx] = BaseCard::None;
                        DragableCard::BaseCard(value)
                    } else {
                        cards[idx].take_from_path(path)
                    }
                } else {
                    panic!("Invalid state");
                }
            }
            BaseCard::StatusEffects(_, effects) => {
                let Some(effect_idx) = path.pop() else {
                    panic!("Invalid state: path is empty");
                };
                if path.is_empty() {
                    let effect = effects[effect_idx as usize].clone();
                    effects[effect_idx as usize] = StatusEffect::None;
                    return DragableCard::StatusEffect(effect);
                }
                effects
                    .get_mut(effect_idx as usize)
                    .unwrap()
                    .take_from_path(path)
            }
            BaseCard::Palette(cards) => {
                let card_idx = path.pop().unwrap() as usize;
                cards[card_idx].clone()
            }
            BaseCard::Effect(Effect::Knockback(_, direction)) => {
                assert!(path.pop().unwrap() == 0);
                let value = DragableCard::Direction(direction.clone());
                *direction = DirectionCard::None;
                value
            }
            invalid_take @ (BaseCard::CreateMaterial(_)
            | BaseCard::None
            | BaseCard::Trigger(_)
            | BaseCard::Effect(_)) => panic!("Invalid state: cannot take from {:?}", invalid_take),
        }
    }

    fn insert_to_path(&mut self, path: &mut Vec<usize>, item: DragableCard) {
        match self {
            BaseCard::Projectile(modifiers) => {
                if path.is_empty() {
                    let DragableCard::ProjectileModifier(item) = item else {
                        panic!("Invalid state")
                    };
                    if let ProjectileModifier::SimpleModify(last_ty, last_s) = item.clone() {
                        let mut combined = false;
                        for modifier in modifiers.iter_mut() {
                            match modifier {
                                ProjectileModifier::SimpleModify(ty, s) => {
                                    if last_ty == *ty {
                                        *s += last_s;
                                        combined = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !combined {
                            modifiers.push(item.clone());
                        }
                    } else {
                        modifiers.push(item.clone());
                    }

                    modifiers.retain(|modifier| match modifier {
                        ProjectileModifier::SimpleModify(_, s) => *s != 0,
                        _ => true,
                    });
                } else {
                    let idx = path.pop().unwrap() as usize;
                    modifiers[idx].insert_to_path(path, item);
                }
            }
            BaseCard::MultiCast(cards, modifiers) => {
                if path.is_empty() {
                    if let DragableCard::BaseCard(item) = item {
                        cards.push(item);
                    } else if let DragableCard::MultiCastModifier(modifier_item) = item {
                        let mut combined = false;
                        match modifier_item.clone() {
                            MultiCastModifier::None => {}
                            MultiCastModifier::Duplication(last_s) => {
                                for modifier in modifiers.iter_mut() {
                                    match modifier {
                                        MultiCastModifier::Duplication(s) => {
                                            *s += last_s;
                                            combined = true;
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            MultiCastModifier::Spread(last_s) => {
                                for modifier in modifiers.iter_mut() {
                                    match modifier {
                                        MultiCastModifier::Spread(s) => {
                                            *s += last_s;
                                            combined = true;
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        if !combined {
                            modifiers.push(modifier_item.clone());
                        }
                    } else {
                        panic!("Invalid state")
                    }
                } else {
                    assert!(path.pop().unwrap() == 1);
                    let idx = path.pop().unwrap() as usize;
                    cards[idx].insert_to_path(path, item)
                }
            }
            BaseCard::StatusEffects(_, effects) => {
                if path.is_empty() {
                    let DragableCard::StatusEffect(item) = item else {
                        panic!("Invalid state")
                    };
                    if let StatusEffect::SimpleStatusEffect(last_ty, last_s) = item.clone() {
                        let mut combined = false;
                        for effect in effects.iter_mut() {
                            match effect {
                                StatusEffect::SimpleStatusEffect(ty, s) => {
                                    if last_ty == *ty {
                                        *s += last_s;
                                        combined = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !combined {
                            effects.push(item.clone());
                        }
                    } else {
                        effects.push(item.clone());
                    }

                    effects.retain(|effect| match effect {
                        StatusEffect::SimpleStatusEffect(_, s) => *s != 0,
                        _ => true,
                    });
                } else {
                    let idx = path.pop().unwrap() as usize;
                    assert!(path.pop().unwrap() == 0);
                    effects[idx].insert_to_path(path, item);
                }
            }
            BaseCard::Effect(Effect::Knockback(_, direction)) => {
                assert!(path.pop().unwrap() == 0);
                if let DragableCard::Direction(new_direction) = item {
                    *direction = new_direction;
                } else {
                    panic!("Invalid state")
                }
            }
            BaseCard::None => {
                assert!(
                    path.is_empty(),
                    "Invalid state: should not have nonempty path {:?} when inserting into None",
                    path
                );
                let DragableCard::BaseCard(item) = item else {
                    panic!("Invalid state")
                };
                *self = item;
            }
            c => panic!("Invalid state: Could not insert into {:?}", c),
        }
    }

    fn cleanup(&mut self, path: &mut Vec<usize>) {
        match self {
            BaseCard::Projectile(modifiers) => {
                if path.len() <= 1 {
                    modifiers.retain(|modifier| match modifier {
                        ProjectileModifier::None => false,
                        _ => true,
                    });
                } else {
                    let idx = path.pop().unwrap() as usize;
                    modifiers[idx].cleanup(path);
                }
            }
            BaseCard::MultiCast(cards, modifiers) => {
                if path.is_empty() {
                    cards.retain(|card| !matches!(card, BaseCard::None));
                } else {
                    let idx_type = path.pop().unwrap();
                    if idx_type == 0 {
                        let idx = path.pop().unwrap() as usize;
                        assert!(path.is_empty());
                        match modifiers[idx] {
                            MultiCastModifier::None => {
                                modifiers.remove(idx);
                            }
                            MultiCastModifier::Duplication(s) => {
                                if s == 0 {
                                    modifiers.remove(idx);
                                }
                            }
                            MultiCastModifier::Spread(s) => {
                                if s == 0 {
                                    modifiers.remove(idx);
                                }
                            }
                        }
                    } else if idx_type == 1 {
                        let idx = path.pop().unwrap() as usize;
                        if path.is_empty() {
                            if matches!(cards[idx], BaseCard::None) {
                                cards.remove(idx);
                            }
                        } else {
                            cards[idx].cleanup(path);
                        }
                    } else {
                        panic!("Invalid state");
                    }
                }
            }
            BaseCard::StatusEffects(_, effects) => {
                if path.len() <= 1 {
                    effects.retain(|effect| match effect {
                        StatusEffect::None => false,
                        _ => true,
                    });
                } else {
                    let idx = path.pop().unwrap() as usize;
                    assert!(path.pop().unwrap() == 0);
                    match &mut effects[idx] {
                        StatusEffect::OnHit(card) => card.cleanup(path),
                        StatusEffect::SimpleStatusEffect(
                            SimpleStatusEffectType::IncreaseGravity(_),
                            _,
                        ) => {}
                        ref invalid => panic!(
                            "Invalid state: cannot follow path {} into {:?}",
                            idx, invalid
                        ),
                    }
                }
            }
            BaseCard::None => {
                assert!(path.is_empty(), "Invalid state");
            }
            BaseCard::Effect(Effect::Knockback(_, _)) => {}
            c => panic!("Invalid state: Could cleanup {:?}", c),
        }
    }
}

impl DragableCard {
    pub fn get_type(&self) -> DragableType {
        match self {
            DragableCard::BaseCard(_) => DragableType::BaseCard,
            DragableCard::CooldownModifier(_) => DragableType::CooldownModifier,
            DragableCard::MultiCastModifier(_) => DragableType::MultiCastModifier,
            DragableCard::ProjectileModifier(_) => DragableType::ProjectileModifier,
            DragableCard::StatusEffect(_) => DragableType::StatusEffect,
            DragableCard::Direction(_) => DragableType::Direction,
        }
    }

    fn draw_draggable(
        &mut self,
        ui: &mut Ui,
        path: &mut Vec<usize>,
        dnd_path: &mut Option<(Location, Location)>,
        modify_path: &mut Option<(Vec<usize>, ModificationType)>,
        edit_mode: &EditMode,
    ) {
        match self {
            DragableCard::BaseCard(card) => {
                card.draw(ui, path, dnd_path, modify_path, &edit_mode)
            }
            DragableCard::CooldownModifier(modifier) => {
                modifier.draw(ui, path, dnd_path, modify_path, edit_mode)
            }
            DragableCard::MultiCastModifier(modifier) => {
                modifier.draw(ui, path, dnd_path, modify_path, edit_mode)
            }
            DragableCard::ProjectileModifier(modifier) => {
                modifier.draw(ui, path, dnd_path, modify_path, edit_mode)
            }
            DragableCard::StatusEffect(effect) => {
                effect.draw(ui, path, dnd_path, modify_path, edit_mode)
            }
            DragableCard::Direction(direction) => {
                direction.draw(ui, path, dnd_path, modify_path, edit_mode)
            }
        }
    }
}

pub fn draw_label(
    ui: &mut Ui,
    name: &str,
    hover_text: String,
    modify_path: &mut Option<(Vec<usize>, ModificationType)>,
    path: &mut Vec<usize>,
) {
    let mut job = LayoutJob::default();
    job.append(
        name,
        0.0,
        TextFormat {
            color: Color32::WHITE,
            ..Default::default()
        },
    );
    let widget = if ui.input(|i| i.modifiers.ctrl) {
        Label::new(job).sense(Sense::click()).selectable(false)
    } else {
        Label::new(job).selectable(false)
    };
    let response = ui.add(widget).on_hover_text(hover_text);

    if response.clicked() {
        if modify_path.is_none() {
            let modification_type = if ui.input(|i| i.modifiers.shift) {
                ModificationType::Remove
            } else {
                ModificationType::Add
            };
            *modify_path = Some((path.clone(), modification_type));
        }
    }
}

fn draw_modifier<T: std::fmt::Display + Numeric + Copy>(
    ui: &mut Ui,
    id: Id,
    name: String,
    count: Option<&mut T>,
    hover_text: String,
    handle_drag: bool,
    modify_path: &mut Option<(Vec<usize>, ModificationType)>,
    path: &mut Vec<usize>,
    edit_mode: &EditMode,
) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    let mut job = LayoutJob::default();
    job.append(
        &name,
        0.0,
        TextFormat {
            color: Color32::WHITE,
            ..Default::default()
        },
    );
    let add_contents = |ui: &mut Ui| {
        ui.style_mut().spacing.interact_size.x = 0.0;
        ui.style_mut().spacing.interact_size.y = 0.0;
        ui.style_mut().spacing.item_spacing.x = 0.0;
        ui.style_mut().spacing.button_padding.x = 1.0;
        ui.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::new(0.0, Color32::WHITE);
        ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.active.bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.active.fg_stroke = Stroke::new(0.0, Color32::WHITE);
        ui.style_mut().visuals.widgets.inactive.fg_stroke = Stroke::new(0.0, Color32::WHITE);
        ui.style_mut().visuals.widgets.noninteractive.fg_stroke =
            Stroke::new(0.0, Color32::WHITE);
        ui.style_mut().text_styles = [
            (
                TextStyle::Heading,
                FontId::new(30.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Name("Heading2".into()),
                FontId::new(25.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Name("Context".into()),
                FontId::new(23.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(18.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(14.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(7.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(10.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();
        let response = ui.add(Label::new(job).selectable(false)).on_hover_text(hover_text);
        if let Some(count) = count {
            ui.vertical(|ui| {
                if edit_mode.can_edit_modifiers() {
                    if ui.add(DragValue::new(count).speed(0.1)).changed() {
                        if modify_path.is_none() {
                            *modify_path = Some((path.clone(), ModificationType::Other));
                        }
                    }
                } else {
                    let mut job = LayoutJob::default();
                    job.append(
                        &count.to_string(),
                        0.0,
                        TextFormat {
                            color: Color32::WHITE,
                            font_id: FontId::new(7.0, egui::FontFamily::Proportional),
                            ..Default::default()
                        },
                    );
                    ui.add(Label::new(job).selectable(false));
                }
            });
        }
        response
    };

    let can_be_dragged = edit_mode.can_drag_modifiers() && handle_drag;

    if can_be_dragged {
        dnd_drag_source::<Location, _>(ui, id, Location { path: path.clone() }, add_contents);
    } else {
        ui.scope(add_contents);
    }

    // if !is_being_dragged || !can_be_dragged {
    //     let prev_frame_area: Option<Rect> = ui.data(|d| d.get_temp(id));
    //     let mut size = vec2(0.0, 0.0);
    //     //load from previous frame
    //     if let Some(area) = prev_frame_area {
    //         if can_be_dragged {
    //             // Check for drags:
    //             let response = ui.interact(area, id, Sense::drag());
    //             if response.hovered() {
    //                 ui.ctx().set_cursor_icon(CursorIcon::Grab);
    //             }
    //         }
    //         size.x = area.size().x;
    //     }
    //     if ui.available_size_before_wrap().x < size.x {
    //         ui.end_row();
    //     }
    //     let response = ui.scope(add_contents).response;

    //     if response.clicked() {
    //         if modify_path.is_none() {
    //             let modification_type = if ui.input(|i| i.modifiers.shift) {
    //                 ModificationType::Remove
    //             } else {
    //                 ModificationType::Add
    //             };
    //             *modify_path = Some((path.clone(), modification_type));
    //         }
    //     }
    //     //store for next frame
    //     ui.data_mut(|d| d.insert_temp(id, response.rect.shrink(CARD_UI_SPACING)));
    // } else {
    //     ui.ctx().set_cursor_icon(CursorIcon::Grabbing);

    //     // Paint the body to a new layer:
    //     let layer_id = LayerId::new(Order::Tooltip, id);
    //     let response = ui.with_layer_id(layer_id, add_contents).response;

    //     // Now we move the visuals of the body to where the mouse is.
    //     // Normally you need to decide a location for a widget first,
    //     // because otherwise that widget cannot interact with the mouse.
    //     // However, a dragged component cannot be interacted with anyway
    //     // (anything with `Order::Tooltip` always gets an empty [`Response`])
    //     // So this is fine!

    //     if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
    //         let delta = pointer_pos - response.rect.center();
    //         ui.ctx().transform_layer_shapes(layer_id, TSTransform::from_translation(delta));
    //     }
    // }
}

pub fn card_editor(
    ctx: &egui::Context,
    gui_state: &mut GuiState,
    // game: &mut Option<Game>
) {
    const PADDING: f32 = 10.0;
    
    egui::CentralPanel::default()
    .frame(Frame::new().inner_margin(PADDING))
        .show(&ctx, |ui| {
            ui.painter()
                .rect_filled(ui.clip_rect(), 0.0, Color32::BLACK);

            let mut edit_mode = 
            // if let Some(game) = game {
            //     if gui_state.render_deck_idx > 0 {
            //         EditMode::Readonly
            //     } else {
            //         game.game_mode.deck_swapping(
            //             game.rollback_data
            //                 .get_players()
            //                 .get(gui_state.render_deck_idx)
            //                 .unwrap(),
            //         )
            //     }
            // } else {
                EditMode::FullEditing;
            // };
            ui.scope(|ui| {
                ui.add_space(PADDING);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Card Editor").color(Color32::WHITE));
                    // if let Some(game) = game {
                    //     egui::ComboBox::from_label("Decks")
                    //         .selected_text(format!("Player {}", gui_state.render_deck_idx))
                    //         .show_ui(ui, |ui| {
                    //             for (idx, metadata) in
                    //                 game.rollback_data.get_entity_metadata().iter().enumerate()
                    //             {
                    //                 if ui
                    //                     .selectable_value(
                    //                         &mut gui_state.render_deck_idx,
                    //                         idx,
                    //                         format!("Player {}", idx),
                    //                     )
                    //                     .clicked()
                    //                 {
                    //                     if let EntityMetaData::Player(deck, _) = metadata {
                    //                         gui_state.render_deck = deck.clone();
                    //                     } else {
                    //                         gui_state.render_deck = Deck {
                    //                             cooldowns: vec![],
                    //                             passive: PassiveCard {
                    //                                 passive_effects: vec![],
                    //                             },
                    //                         };
                    //                     }
                    //                     edit_mode = if gui_state.render_deck_idx > 0 {
                    //                         EditMode::Readonly
                    //                     } else {
                    //                         game.game_mode.deck_swapping(
                    //                             game.rollback_data
                    //                                 .get_players()
                    //                                 .get(gui_state.render_deck_idx)
                    //                                 .unwrap(),
                    //                         )
                    //                     }
                    //                 }
                    //             }
                    //         });
                    // }
                    if ui.button("Export to Clipboard").clicked() {
                        let export = ron::to_string(&gui_state.render_deck).unwrap();
                        ctx.copy_text(export);
                    }

                    if matches!(edit_mode, EditMode::FullEditing) {
                        // if ui.button("Import from Clipboard").clicked() {
                        //     let mut clipboard = clippers::Clipboard::get();
                        //     let import: Option<Deck> = match clipboard.read() {
                        //         Some(clippers::ClipperData::Text(text)) => {
                        //             let clipboard_parse = ron::from_str(text.as_str());
                        //             if let Err(e) = &clipboard_parse {
                        //                 gui_state
                        //                     .errors
                        //                     .push(format!("Failed to parse clipboard: {}", e));
                        //                 println!("Failed to parse clipboard: {}", e);
                        //             }
                        //             clipboard_parse.ok()
                        //         }
                        //         _ => {
                        //             println!("Failed to import from clipboard");
                        //             None
                        //         }
                        //     };
                        //     if let Some(import) = import {
                        //         gui_state.render_deck = import;
                        //     }
                        // }

                        if ui.button("Clear Dock").clicked() {
                            gui_state.dock_cards = vec![];
                        }
                    }
                });

                if matches!(edit_mode, EditMode::FullEditing) {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::BaseCards,
                            "Base Cards",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::Materials,
                            "Materials",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::ProjectileModifiers,
                            "Projectile Modifiers",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::AdvancedProjectileModifiers,
                            "Advanced Projectile Modifiers",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::MultiCastModifiers,
                            "Multicast Modifiers",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::StatusEffects,
                            "Status Effects",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::CooldownModifiers,
                            "Cooldown Modifiers",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::Directions,
                            "Directions",
                        );
                        ui.selectable_value(
                            &mut gui_state.palette_state,
                            PaletteState::Dock,
                            "Dock",
                        );
                    });
                }
            });

            let mut dnd_path = None;
            let mut modify_path = None;
            let mut palette_card = BaseCard::Palette(match gui_state.palette_state {
                PaletteState::ProjectileModifiers => vec![
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Gravity,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Health,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Length,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Width,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Height,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Size,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Speed,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::SimpleModify(
                        SimpleProjectileModifierType::Lifetime,
                        1,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::NoEnemyFire),
                    DragableCard::ProjectileModifier(ProjectileModifier::FriendlyFire),
                    DragableCard::ProjectileModifier(ProjectileModifier::LockToOwner(
                        DirectionCard::None,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::PiercePlayers),
                ],
                PaletteState::BaseCards => vec![
                    DragableCard::BaseCard(BaseCard::Projectile(vec![])),
                    DragableCard::BaseCard(BaseCard::MultiCast(vec![], vec![])),
                    DragableCard::BaseCard(BaseCard::Trigger(0)),
                    DragableCard::BaseCard(BaseCard::Effect(Effect::Damage(1))),
                    DragableCard::BaseCard(BaseCard::Effect(Effect::Knockback(
                        1,
                        DirectionCard::None,
                    ))),
                    DragableCard::BaseCard(BaseCard::Effect(Effect::Cleanse)),
                    DragableCard::BaseCard(BaseCard::Effect(Effect::Teleport)),
                    DragableCard::BaseCard(BaseCard::StatusEffects(1, vec![])),
                ],
                PaletteState::AdvancedProjectileModifiers => vec![
                    DragableCard::ProjectileModifier(ProjectileModifier::OnHit(BaseCard::None)),
                    DragableCard::ProjectileModifier(ProjectileModifier::OnHeadshot(
                        BaseCard::None,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::OnExpiry(
                        BaseCard::None,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::OnTrigger(
                        0,
                        BaseCard::None,
                    )),
                    DragableCard::ProjectileModifier(ProjectileModifier::Trail(
                        1,
                        BaseCard::None,
                    )),
                ],
                PaletteState::MultiCastModifiers => vec![
                    DragableCard::MultiCastModifier(MultiCastModifier::Spread(1)),
                    DragableCard::MultiCastModifier(MultiCastModifier::Duplication(1)),
                ],
                PaletteState::CooldownModifiers => vec![
                    DragableCard::CooldownModifier(CooldownModifier::SimpleCooldownModifier(
                        SimpleCooldownModifier::AddCharge,
                        1,
                    )),
                    DragableCard::CooldownModifier(CooldownModifier::SimpleCooldownModifier(
                        SimpleCooldownModifier::AddCooldown,
                        1,
                    )),
                    DragableCard::CooldownModifier(
                        CooldownModifier::SignedSimpleCooldownModifier(
                            SignedSimpleCooldownModifier::DecreaseCooldown,
                            1,
                        ),
                    ),
                    DragableCard::CooldownModifier(CooldownModifier::Reloading),
                ],
                PaletteState::StatusEffects => vec![
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::DamageOverTime,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::IncreaseDamageTaken,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::IncreaseGravity(DirectionCard::None),
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::Speed,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::UnsignedSimpleStatusEffect(
                        UnsignedSimpleStatusEffectType::Overheal,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::Grow,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::SimpleStatusEffect(
                        SimpleStatusEffectType::IncreaseMaxHealth,
                        1,
                    )),
                    DragableCard::StatusEffect(StatusEffect::Invincibility),
                    DragableCard::StatusEffect(StatusEffect::Trapped),
                    DragableCard::StatusEffect(StatusEffect::Lockout),
                    DragableCard::StatusEffect(StatusEffect::Stun),
                    DragableCard::StatusEffect(StatusEffect::OnHit(Box::new(BaseCard::None))),
                ],
                PaletteState::Materials => vec![
                    DragableCard::BaseCard(BaseCard::CreateMaterial(VoxelMaterial::Grass)),
                    DragableCard::BaseCard(BaseCard::CreateMaterial(VoxelMaterial::Dirt)),
                    DragableCard::BaseCard(BaseCard::CreateMaterial(VoxelMaterial::Stone)),
                    DragableCard::BaseCard(BaseCard::CreateMaterial(VoxelMaterial::Ice)),
                    DragableCard::BaseCard(BaseCard::CreateMaterial(VoxelMaterial::Water)),
                ],
                PaletteState::Directions => vec![
                    DragableCard::Direction(DirectionCard::Up),
                    DragableCard::Direction(DirectionCard::Forward),
                    DragableCard::Direction(DirectionCard::Movement),
                ],
                PaletteState::Dock => gui_state.dock_cards.clone(),
            });

            if matches!(edit_mode, EditMode::FullEditing) {
                ui.scope(|ui| {
                    ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                    palette_card.draw(
                        ui,
                        &mut vec![0].into(),
                        &mut dnd_path,
                        &mut modify_path,
                        &edit_mode,
                    );
                });
            }
            ui.separator();
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_bar_visibility(
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                )
                .scroll_source(ScrollSource::ALL)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        let total_impact = gui_state.render_deck.get_total_impact();

                        ui.horizontal_top(|ui| {
                            gui_state.render_deck.passive.draw(
                                ui,
                                &mut vec![1].into(),
                                &mut dnd_path,
                                &mut modify_path,
                                &edit_mode,
                            );
                        });

                        for (ability_idx, cooldown) in
                            gui_state.render_deck.cooldowns.iter_mut().enumerate()
                        {
                            if cooldown.cooldown_value.is_none() {
                                cooldown.cooldown_value =
                                    Some(cooldown.get_cooldown_recovery(total_impact));
                            }
                            ui.horizontal_top(|ui| {
                                cooldown.draw(
                                    ui,
                                    &mut vec![ability_idx + 2].into(),
                                    &mut dnd_path,
                                    &mut modify_path,
                                    &edit_mode,
                                );
                            });
                        }

                        if matches!(edit_mode, EditMode::FullEditing) {
                            if ui
                                .button("Add Cooldown")
                                .on_hover_text("Add a new cooldown")
                                .clicked()
                            {
                                gui_state.render_deck.cooldowns.push(Cooldown::empty());
                            }
                        }

                        if let Some((mut modify_path, modification_type)) = modify_path {
                            for cooldown in gui_state.render_deck.cooldowns.iter_mut() {
                                cooldown.cooldown_value = None;
                            }
                            let modify_action_idx = modify_path.pop().unwrap() as usize;
                            if modify_action_idx == 1 {
                                gui_state.render_deck.passive.modify_from_path(
                                    &mut modify_path.clone(),
                                    modification_type,
                                );
                            } else if modify_path.is_empty() {
                                if matches!(modification_type, ModificationType::Remove) {
                                    gui_state
                                        .render_deck
                                        .cooldowns
                                        .remove(modify_action_idx - 2);
                                }
                            } else if modify_action_idx > 1 {
                                gui_state.render_deck.cooldowns[modify_action_idx - 2]
                                    .modify_from_path(
                                        &mut modify_path.clone(),
                                        modification_type,
                                    );
                            }
                        }
                        if let Some((Location { path: source_path }, Location { path: drop_path })) = dnd_path.as_mut() {
                            source_path.reverse();
                            drop_path.reverse();

                            let source_type = &DragableType::ProjectileModifier;
                            let drop_type = &DropableType::BaseProjectile;


                            for cooldown in gui_state.render_deck.cooldowns.iter_mut() {
                                cooldown.cooldown_value = None;
                            }
                            let source_action_idx =
                                source_path.pop().unwrap() as usize;
                            let drop_action_idx = drop_path.pop().unwrap() as usize;
                            if ui.input(|i| i.pointer.any_released())
                                && (is_valid_drag(source_type, drop_type)
                                    || drop_action_idx == 0)
                            {
                                // do the drop:
                                let item = if source_action_idx == 0 {
                                    palette_card.take_from_path(source_path)
                                } else if source_action_idx == 1 {
                                    gui_state
                                        .render_deck
                                        .passive
                                        .take_from_path(&mut source_path.clone())
                                } else {
                                    gui_state.render_deck.cooldowns[source_action_idx - 2]
                                        .take_from_path(&mut source_path.clone())
                                };
                                if drop_action_idx == 1 {
                                    gui_state
                                        .render_deck
                                        .passive
                                        .insert_to_path(drop_path, item);
                                } else if drop_action_idx > 1 {
                                    gui_state.render_deck.cooldowns[drop_action_idx - 2]
                                        .insert_to_path(drop_path, item);
                                } else if matches!(
                                    gui_state.palette_state,
                                    PaletteState::Dock
                                ) {
                                    gui_state.dock_cards.push(item);
                                }
                                if source_action_idx == 1 {
                                    gui_state.render_deck.passive.cleanup(source_path);
                                } else if source_action_idx > 1 {
                                    gui_state.render_deck.cooldowns[source_action_idx - 2]
                                        .cleanup(source_path);
                                }
                            }
                        }
                    });
                });
            if !matches!(edit_mode, EditMode::Readonly) {
                gui_state.gui_deck = gui_state.render_deck.clone();
            }
        });
}
