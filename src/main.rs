mod egui_tools;
mod gui;
mod lobby_browser;
mod settings_manager;
mod shared;

use slang_playground_compiler::CompilationResult;
use slang_renderer::Renderer;
use std::{fs, sync::Arc};
use wgpu::Features;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[cfg(target_family = "wasm")]
use std::panic;
#[cfg(debug_assertions)]
#[cfg(target_family = "wasm")]
extern crate console_error_panic_hook;

#[cfg(not(target_arch = "wasm32"))]
use slang_debug_app::DebugAppState;

use crate::{
    gui::{GuiElement, GuiState, PaletteState, horizontal_centerer, vertical_centerer},
    lobby_browser::LobbyBrowser,
    settings_manager::{Control, Settings},
};

struct RenderData {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub egui_renderer: egui_tools::EguiRenderer,
}

impl RenderData {
    async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        let info = adapter.get_info();

        let info_logging = format!("Running on backend: {}\n", info.backend);
        #[cfg(not(target_arch = "wasm32"))]
        print!("{}", info_logging);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&info_logging.into());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                required_limits: wgpu::Limits {
                    max_storage_buffers_per_shader_stage: 9, // this should be fixed later for compat
                    ..wgpu::Limits::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_format = wgpu::TextureFormat::Rgba8Unorm;
        configure_surface(&surface, &device, window.inner_size(), surface_format);

        let egui_renderer =
            egui_tools::EguiRenderer::new(&device, surface_format, None, 1, &window);

        RenderData {
            window: window,
            surface: surface,
            egui_renderer,
            device: device,
            queue: queue,
        }
    }
}

// Surface configuration is now handled externally if needed
fn configure_surface(
    surface: &wgpu::Surface,
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    surface_format: wgpu::TextureFormat,
) {
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        view_formats: vec![surface_format],
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        width: size.width,
        height: size.height,
        desired_maximum_frame_latency: 2,
        present_mode: if cfg!(target_arch = "wasm32") {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::Immediate
        },
    };
    surface.configure(device, &surface_config);
}

const SETTINGS_FILE: &str = "settings.yaml";

struct App {
    render_data: Option<RenderData>,
    game: Option<Renderer>,
    player_input: playground_module::PlayerInput,
    #[cfg(target_arch = "wasm32")]
    state_receiver: Option<futures::channel::oneshot::Receiver<RenderData>>,
    #[cfg(not(target_arch = "wasm32"))]
    debug_app: Option<DebugAppState>,
    compilation: Option<CompilationResult>,
    surface_format: wgpu::TextureFormat,
    settings: Settings,
    gui_state: GuiState,
}

impl App {
    fn new(compilation: CompilationResult) -> Self {
        let settings = Settings::from_string(fs::read_to_string(SETTINGS_FILE).unwrap().as_str());

        let gui_state = GuiState {
            menu_stack: vec![GuiElement::MainMenu],
            errors: Vec::new(),
            // gui_deck: player_deck.clone(),
            // render_deck: player_deck.clone(),
            // dock_cards: vec![],
            render_deck_idx: 0,
            cooldown_cache_refresh_delay: 0.0,
            palette_state: PaletteState::BaseCards,
            should_exit: false,
            game_just_started: false,
            lobby_browser: LobbyBrowser::new(),
        };

        Self {
            render_data: None,
            game: None,
            player_input: playground_module::PlayerInput {
                forward: 0.0,
                backward: 0.0,
                left: 0.0,
                right: 0.0,
                jump: 0.0,
                crouch: 0.0,
                interact: 0.0,
            },
            #[cfg(target_arch = "wasm32")]
            state_receiver: None,
            #[cfg(not(target_arch = "wasm32"))]
            debug_app: None,
            compilation: Some(compilation),
            surface_format: wgpu::TextureFormat::Rgba8Unorm,
            settings,
            gui_state,
        }
    }

    fn render_frame(&mut self) {
        let Some(render_data) = self.render_data.as_mut() else {
            return;
        };

        let surface_texture = render_data
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_format),
                ..Default::default()
            });
        let mut encoder = render_data
            .device
            .create_command_encoder(&Default::default());

        if let Some(game) = self.game.as_mut() {
            playground_module::set_player_input(game, self.player_input);
            game.begin_frame();
            game.run_compute_passes(&mut encoder);
            game.run_draw_passes(&mut encoder, &texture_view);
        }

        {
            use egui::*;

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [
                    surface_texture.texture.width(),
                    surface_texture.texture.height(),
                ],
                pixels_per_point: render_data.window.scale_factor() as f32,
            };

            let window_size = render_data.window.inner_size();
            let device = render_data.device.clone();
            let queue = render_data.queue.clone();

            render_data.egui_renderer.begin_frame(&render_data.window);

            let ctx = render_data.egui_renderer.context();
            match self.gui_state.menu_stack.last() {
                Some(&GuiElement::MainMenu) => {
                    egui::Area::new("main menu".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        if ui.button("Singleplayer").clicked() {
                                            self.gui_state
                                                .menu_stack
                                                .push(GuiElement::SingleplayerMenu);
                                        }
                                        if ui.button("Multiplayer").clicked() {
                                            self.gui_state
                                                .menu_stack
                                                .push(GuiElement::MultiplayerMenu);
                                        }
                                        if ui.button("Deck Picker").clicked() {
                                            self.gui_state.menu_stack.push(GuiElement::DeckPicker);
                                        }
                                        // if ui.button("Play Replay").clicked() {
                                        //     let mut replay_folder_path =
                                        //         std::env::current_dir().unwrap();
                                        //     replay_folder_path.push(
                                        //         self.settings.replay_settings.replay_folder.clone(),
                                        //     );
                                        //     let file = FileDialog::new()
                                        //         .add_filter("replay", &["replay"])
                                        //         .set_directory(replay_folder_path)
                                        //         .pick_file();
                                        //     if let Some(file) = file {
                                        //         self.gui_state.menu_stack.pop();
                                        //         *game = Some(Game::from_replay(
                                        //             file.as_path(),
                                        //             creation_interface,
                                        //         ));
                                        //         self.gui_state.game_just_started = true;
                                        //     }
                                        // }
                                        if ui.button("Card Editor").clicked() {
                                            self.gui_state.menu_stack.push(GuiElement::CardEditor);
                                        }
                                        if ui.button("Exit to Desktop").clicked() {
                                            self.gui_state.should_exit = true;
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(&GuiElement::SingleplayerMenu) => {
                    egui::Area::new("singleplayer menu".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        for preset in self.settings.preset_settings.iter() {
                                            if ui.button(&preset.name).clicked() {
                                                self.gui_state.menu_stack.clear();
                                                self.game =
                                                    Some(pollster::block_on(Renderer::new(
                                                        self.compilation.clone().unwrap(),
                                                        window_size,
                                                        device.clone(),
                                                        queue.clone(),
                                                    )));
                                                self.gui_state.game_just_started = true;
                                            }
                                        }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(&GuiElement::EscMenu) => {
                    egui::Area::new("menu".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            ui.painter().rect_filled(
                                ui.available_rect_before_wrap(),
                                0.0,
                                Color32::BLACK.gamma_multiply(0.5),
                            );

                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                egui::vec2(300.0, 300.0),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("Menu").color(Color32::WHITE));
                                    if ui.button("Card Editor").clicked() {
                                        self.gui_state.menu_stack.push(GuiElement::CardEditor);
                                    }
                                    // if let Some(game) = game {
                                    //     if game.game_mode.has_mode_gui() {
                                    //         if ui.button("Mode configuration").clicked() {
                                    //             self.gui_state.menu_stack.push(GuiElement::ModeGui);
                                    //         }
                                    //     }
                                    // }
                                    if ui.button("Leave Game").clicked() {
                                        // if let Some(game) = game {
                                        //     game.rollback_data.leave_game();
                                        // }
                                        self.gui_state.menu_stack.clear();
                                        self.gui_state.menu_stack.push(GuiElement::MainMenu);
                                        self.game = None;
                                    }
                                    if ui.button("Exit to Desktop").clicked() {
                                        self.gui_state.should_exit = true;
                                    }
                                });
                            });
                        });
                }
                Some(&GuiElement::CardEditor) => {
                    // card_editor(&ctx, gui_state, game);
                }

                Some(&GuiElement::MultiplayerMenu) => {
                    egui::Area::new("multiplayer menu".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        if ui.button("Host").clicked() {
                                            let client = reqwest::Client::new();
                                            let new_lobby_response = pollster::block_on(
                                                client
                                                    .post(format!(
                                                        "http://{}create_lobby",
                                                        self.settings.remote_url.clone()
                                                    ))
                                                    .json(&self.settings.create_lobby_settings)
                                                    .send(),
                                            );
                                            let new_lobby_response = match new_lobby_response {
                                                Ok(new_lobby_response) => new_lobby_response,
                                                Err(e) => {
                                                    println!("error creating lobby: {:?}", e);
                                                    self.gui_state.errors.push(
                                                        format!("Error creating lobby {}", e)
                                                            .to_string(),
                                                    );
                                                    return;
                                                }
                                            };
                                            let new_lobby_id = pollster::block_on(new_lobby_response.json::<String>());
                                            let new_lobby_id = match new_lobby_id {
                                                Ok(new_lobby_id) => new_lobby_id,
                                                Err(e) => {
                                                    println!("error creating lobby: {:?}", e);
                                                    self.gui_state.errors.push(
                                                        format!("Error creating lobby {}", e)
                                                            .to_string(),
                                                    );
                                                    return;
                                                }
                                            };
                                            println!("new lobby id: {}", new_lobby_id);
                                            self.game = Some(pollster::block_on(Renderer::new(
                                                self.compilation.clone().unwrap(),
                                                window_size,
                                                device.clone(),
                                                queue.clone(),
                                            )));
                                            self.gui_state.menu_stack.push(GuiElement::LobbyQueue);
                                            self.gui_state.game_just_started = true;
                                        }
                                        if ui.button("Join").clicked() {
                                            self.gui_state.lobby_browser.update(&self.settings);
                                            self.gui_state
                                                .menu_stack
                                                .push(GuiElement::LobbyBrowser);
                                        }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(&GuiElement::LobbyBrowser) => {
                    use egui_extras::{Column, TableBuilder};
                    egui::Area::new("lobby browser".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );
                            let lobby_list = match self.gui_state.lobby_browser.get_lobbies()
                            {
                                Ok(lobby_list) => lobby_list,
                                Err(err) => {
                                    self.gui_state.errors.push(format!(
                                        "Error getting lobbies: {}",
                                        err
                                    ));
                                    vec![]
                                }
                            };

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        horizontal_centerer(ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                let available_height = ui.available_height();
                                                let table = TableBuilder::new(ui)
                                                    .striped(true)
                                                    .resizable(false)
                                                    .cell_layout(egui::Layout::left_to_right(
                                                        egui::Align::Center,
                                                    ))
                                                    .column(Column::auto())
                                                    .column(Column::auto())
                                                    .column(Column::auto())
                                                    .column(Column::auto())
                                                    .column(Column::auto())
                                                    .max_scroll_height(available_height);

                                                table
                                                    .header(20.0, |mut header| {
                                                        header.col(|ui| {
                                                            ui.strong("Name");
                                                        });
                                                        header.col(|ui| {
                                                            ui.strong("Mode");
                                                        });
                                                        header.col(|ui| {
                                                            ui.strong("Map");
                                                        });
                                                        header.col(|ui| {
                                                            ui.strong("Players");
                                                        });
                                                        header.col(|ui| {
                                                            ui.strong("");
                                                        });
                                                    })
                                                    .body(|mut body| {
                                                        for lobby in lobby_list.iter() {
                                                            body.row(20.0, |mut row| {
                                                                row.col(|ui| {
                                                                    ui.label(lobby.name.clone());
                                                                });
                                                                row.col(|ui| {
                                                                    ui.label(
                                                                        lobby
                                                                            .settings
                                                                            .game_mode
                                                                            .get_name(),
                                                                    );
                                                                });
                                                                row.col(|ui| {
                                                                    ui.label(
                                                                        lobby
                                                                            .settings
                                                                            .world_gen
                                                                            .get_name(),
                                                                    );
                                                                });
                                                                row.col(|ui| {
                                                                    ui.label(format!(
                                                                        "{}/{}",
                                                                        lobby.player_count,
                                                                        lobby.settings.player_count
                                                                    ));
                                                                });
                                                                row.col(|ui| {
                                                                    if ui.button("Join").clicked() {
                                                                        self.gui_state
                                                                            .menu_stack
                                                                            .clear();
                                                                        self.game = Some(pollster::block_on(Renderer::new(
                                                                            self.compilation.clone().unwrap(),
                                                                            window_size,
                                                                            device.clone(),
                                                                            queue.clone(),
                                                                        )));
                                                                        self.gui_state.menu_stack.push(
                                                                            GuiElement::LobbyQueue,
                                                                        );
                                                                        self.gui_state
                                                                            .game_just_started =
                                                                            true;
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    });
                                            });
                                        });
                                        if lobby_list.is_empty() {
                                            ui.label("No lobbies found");
                                        }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(&GuiElement::LobbyQueue) => {
                    egui::Area::new("lobby queue".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label("Waiting for players to join...");
                                        // if let Some(game) = game {
                                        //     ui.label(format!(
                                        //         "Players: {}/{}",
                                        //         game.rollback_data.player_count(),
                                        //         game.game_settings.player_count
                                        //     ));
                                        // }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                            self.game = None;
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(&GuiElement::ModeGui) => {
                    egui::Area::new("mode gui".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        // if let Some(game) = game {
                                        //     game.game_mode.mode_gui(ui, &mut game.rollback_data);
                                        // }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                            self.game = None;
                                        }
                                    });
                                });
                            });
                        });
                }
                Some(GuiElement::DeckPicker) => {
                    egui::Area::new("deck picker".into())
                        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                        .show(&ctx, |ui| {
                            let menu_size = Rect::from_center_size(
                                ui.available_rect_before_wrap().center(),
                                ui.available_rect_before_wrap().size(),
                            );

                            ui.scope_builder(UiBuilder::new().max_rect(menu_size), |ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    Color32::BLACK,
                                );
                                vertical_centerer(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        // let Ok(decks) = recurse_files(self.settings.card_dir.clone()) else {
                                        //     panic!("Cannot read directory {}", self.settings.card_dir);
                                        // };
                                        // for deck in decks {
                                        //     if ui.button(deck.file_name().unwrap().to_str().unwrap()).clicked() {
                                        //         self.gui_state.gui_deck = ron::from_str(fs::read_to_string(deck.as_path()).unwrap().as_str()).unwrap();
                                        //         self.gui_state.menu_stack.pop();
                                        //         self.gui_state.menu_stack.push(GuiElement::CardEditor);
                                        //     }
                                        // }
                                        if ui.button("Back").clicked() {
                                            self.gui_state.menu_stack.pop();
                                        }
                                    });
                                });
                            });
                        });
                }
                None => {
                    egui::Area::new("crosshair".into())
                        .anchor(Align2::CENTER_CENTER, (0.0, 0.0))
                        .show(&ctx, |ui| {
                            let center = ui.available_rect_before_wrap().center();

                            // if spectate_player.hitmarker.0 + spectate_player.hitmarker.1 > 0.0 {
                            //     let hitmarker_size = 0.5 * spectate_player.hitmarker.0;
                            //     let head_hitmarker_size = 0.5
                            //         * (spectate_player.hitmarker.0
                            //             + spectate_player.hitmarker.1);
                            //     let hitmarker_thickness = 1.5;
                            //     let head_hitmarker_color = Color32::RED;
                            //     let hitmarker_color = Color32::from_additive_luminance(255);
                            //     ui.painter().add(epaint::Shape::line_segment(
                            //         [
                            //             center
                            //                 + vec2(-head_hitmarker_size, -head_hitmarker_size),
                            //             center + vec2(head_hitmarker_size, head_hitmarker_size),
                            //         ],
                            //         Stroke::new(hitmarker_thickness, head_hitmarker_color),
                            //     ));
                            //     ui.painter().add(epaint::Shape::line_segment(
                            //         [
                            //             center
                            //                 + vec2(-head_hitmarker_size, head_hitmarker_size),
                            //             center
                            //                 + vec2(head_hitmarker_size, -head_hitmarker_size),
                            //         ],
                            //         Stroke::new(hitmarker_thickness, head_hitmarker_color),
                            //     ));
                            //     ui.painter().add(epaint::Shape::line_segment(
                            //         [
                            //             center + vec2(-hitmarker_size, -hitmarker_size),
                            //             center + vec2(hitmarker_size, hitmarker_size),
                            //         ],
                            //         Stroke::new(hitmarker_thickness, hitmarker_color),
                            //     ));
                            //     ui.painter().add(epaint::Shape::line_segment(
                            //         [
                            //             center + vec2(-hitmarker_size, hitmarker_size),
                            //             center + vec2(hitmarker_size, -hitmarker_size),
                            //         ],
                            //         Stroke::new(hitmarker_thickness, hitmarker_color),
                            //     ));
                            // }

                            let thickness = 1.0;
                            let color = Color32::from_additive_luminance(255);
                            let crosshair_size = 10.0;

                            ui.painter().add(epaint::Shape::line_segment(
                                [
                                    center + vec2(-crosshair_size, 0.0),
                                    center + vec2(crosshair_size, 0.0),
                                ],
                                Stroke::new(thickness, color),
                            ));
                            ui.painter().add(epaint::Shape::line_segment(
                                [
                                    center + vec2(0.0, -crosshair_size),
                                    center + vec2(0.0, crosshair_size),
                                ],
                                Stroke::new(thickness, color),
                            ));

                            // //draw hurtmarkers
                            // for (hurt_direction, hurt_size, remaining_marker_duration) in
                            //     spectate_player.hurtmarkers.iter()
                            // {
                            //     let hurtmarker_color = Color32::RED
                            //         .gamma_multiply(remaining_marker_duration / 1.5);
                            //     let hurtmarker_size = 1.2 * hurt_size.sqrt();
                            //     let transformed_hurt_angle = spectate_player.facing[0]
                            //         - (-hurt_direction.z).atan2(hurt_direction.x);
                            //     let hurtmarker_center = center
                            //         + vec2(
                            //             transformed_hurt_angle.cos(),
                            //             transformed_hurt_angle.sin(),
                            //         ) * 50.0;
                            //     ui.painter().circle_filled(
                            //         hurtmarker_center,
                            //         hurtmarker_size,
                            //         hurtmarker_color,
                            //     );
                            // }
                        });
                }
            }

            render_data.egui_renderer.end_frame_and_draw(
                &render_data.device,
                &render_data.queue,
                &mut encoder,
                &render_data.window,
                &texture_view,
                screen_descriptor,
            );
        }

        render_data.queue.submit([encoder.finish()]);
        if let Some(game) = self.game.as_mut() {
            game.handle_print_output();
        }
        surface_texture.present();
    }

    #[cfg(target_arch = "wasm32")]
    fn ensure_state_is_loaded(&mut self) -> bool {
        if self.render_data.is_some() {
            return true;
        }

        if let Some(receiver) = self.state_receiver.as_mut() {
            if let Ok(Some(state)) = receiver.try_recv() {
                self.render_data = Some(state);
                self.state_receiver = None;
            }
        }
        self.render_data.is_some()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut builder = Window::default_attributes().with_title("Slang Native Playground");

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .expect("error window")
                .document()
                .expect("error document")
                .get_element_by_id("canvas")
                .expect("could not find id canvas")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("error HtmlCanvasElement");
            builder = builder.with_decorations(false).with_canvas(Some(canvas));
        }

        let window = Arc::new(event_loop.create_window(builder).unwrap());
        let future = RenderData::new(window);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.render_data = Some(pollster::block_on(future));
        }

        #[cfg(target_arch = "wasm32")]
        {
            let (sender, receiver) = futures::channel::oneshot::channel();
            self.state_receiver = Some(receiver);
            wasm_bindgen_futures::spawn_local(async move {
                let state = future.await;
                if sender.send(state).is_err() {
                    panic!("Failed to create and send renderer!");
                }
            });
        }

        // #[cfg(not(target_arch = "wasm32"))]
        // if cfg!(debug_assertions) {
        //     let debug_state = pollster::block_on(DebugAppState::new(
        //         event_loop,
        //         (1360, 768),
        //         self.render_data.as_ref().unwrap().state.uniform_components.clone(),
        //     ));
        //     self.debug_app = Some(debug_state);
        //     self.render_data.as_ref().unwrap().window.focus_window();
        // }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
                return;
            }
            _ => (),
        }
        #[cfg(target_arch = "wasm32")]
        if !self.ensure_state_is_loaded() {
            return; // Still loading, skip event
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(debug_app) = self.debug_app.as_mut()
            && debug_app.get_window_id() == _window_id
        {
            debug_app.handle_input(&event);
            return;
        }

        #[cfg(target_arch = "wasm32")]
        match event {
            WindowEvent::RedrawRequested => {
                self.render_frame();
            }
            _ => (),
        }

        let Some(render_data) = self.render_data.as_mut() else {
            return;
        };
        match event {
            WindowEvent::Resized(size) => {
                configure_surface(
                    &render_data.surface,
                    &render_data.device,
                    size,
                    self.surface_format,
                );
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key,
                        ..
                    },
                ..
            } => {
                let state_value = if state == ElementState::Pressed {
                    1.0
                } else {
                    0.0
                };
                macro_rules! key_match {
                    ($property:ident) => {
                        if let Control::Key(key_code) = self.settings.movement_controls.$property
                            && key_code == physical_key
                        {
                            self.player_input.$property = state_value;
                        }
                    };
                }
                key_match!(jump);
                key_match!(crouch);
                key_match!(right);
                key_match!(left);
                key_match!(forward);
                key_match!(backward);

                match physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        if state == ElementState::Released {
                            if self.gui_state.menu_stack.len() > 0
                                && !self
                                    .gui_state
                                    .menu_stack
                                    .last()
                                    .is_some_and(|gui| *gui == GuiElement::MainMenu)
                            {
                                let exited_ui = self.gui_state.menu_stack.pop().unwrap();
                                // match exited_ui {
                                //     GuiElement::CardEditor => {
                                //         self.gui_state.render_deck_idx = 0;
                                //         self.gui_state.render_deck = self.gui_state.gui_deck.clone();
                                //         let config = ron::ser::PrettyConfig::default();
                                //         let export = ron::ser::to_string_pretty(
                                //             &self.gui_state.gui_deck,
                                //             config,
                                //         )
                                //         .unwrap();
                                //         fs::write(&self.settings.card_file, export)
                                //             .expect("failed to write card file");
                                //     }
                                //     _ => (),
                                // }
                            } else {
                                self.gui_state.menu_stack.push(GuiElement::EscMenu);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => (),
        }

        render_data
            .egui_renderer
            .handle_input(&render_data.window, &event);
        if let Some(game) = self.game.as_mut() {
            game.process_event(&event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if !self.ensure_state_is_loaded() {
            return; // Still loading, skip event
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.render_frame();
        #[cfg(target_arch = "wasm32")]
        self.render_data.as_ref().unwrap().window.request_redraw();

        #[cfg(not(target_arch = "wasm32"))]
        // Only handle debug window if in debug mode
        if cfg!(debug_assertions) {
            if let Some(debug_app) = self.debug_app.as_mut() {
                debug_app.about_to_wait();
            }
        }
    }
}

#[cfg(target_family = "wasm")]
mod wasm_workaround {
    unsafe extern "C" {
        pub(super) fn __wasm_call_ctors();
    }
}

fn main() {
    // https://github.com/rustwasm/wasm-bindgen/issues/4446
    #[cfg(target_family = "wasm")]
    unsafe {
        wasm_workaround::__wasm_call_ctors()
    };

    #[cfg(debug_assertions)]
    #[cfg(target_family = "wasm")]
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    // wgpu uses `log` for all of our logging, so we initialize a logger with the `env_logger` crate.
    //
    // To change the log level, set the `RUST_LOG` environment variable. See the `env_logger`
    // documentation for more information.
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

    // When the current loop iteration finishes, suspend the thread until
    // another event arrives. Helps keeping CPU utilization low if nothing
    // is happening, which is preferred if the application might be idling in
    // the background.
    // event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new(playground_module::COMPILATION_RESULT.clone());
    event_loop.run_app(&mut app).unwrap();
}

mod playground_module {
    slang_shader_macros::shader_module!("user.slang", ["shaders"]);
}
