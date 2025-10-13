use crate::{
    context::Context,
    interop::{ClientMessage, ServerMessage},
    menu::loading_screen::LoadingScreen,
    render::{mask::MaskedStack, util::UtilRender},
    ui::{layout::AreaOps, *},
};

use geng::prelude::*;
use geng_utils::conversions::Vec2RealConversions;

pub struct MainMenu {
    context: Context,
    ui_context: UiContext,
    ui: MainMenuUi,
    mask_stack: MaskedStack,
    util_render: UtilRender,

    connect: Option<String>,
    test: Option<usize>,
    state: MainMenuState,
}

pub struct MainMenuState {
    action: Option<Action>,
}

enum Action {
    CreateRoom,
    Join(String),
}

pub struct MainMenuUi {}

impl MainMenu {
    pub async fn new(context: &Context, connect: Option<String>, test: Option<usize>) -> Self {
        context.music.play_music(&context.assets.get().sounds.music);
        Self {
            context: context.clone(),
            ui_context: UiContext::new(context),
            ui: MainMenuUi::new(),
            mask_stack: MaskedStack::new(context),
            util_render: UtilRender::new(context.clone()),

            connect,
            test,
            state: MainMenuState { action: None },
        }
    }
}

impl geng::State for MainMenu {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;
        self.ui_context.update(delta_time);
    }

    fn handle_event(&mut self, event: geng::Event) {
        match event {
            geng::Event::CursorMove { position } => {
                self.ui_context.cursor.cursor_move(position.as_f32());
            }
            geng::Event::Wheel { delta } => {
                self.ui_context.cursor.scroll += delta as f32;
            }
            geng::Event::EditText(text) => {
                if self.ui_context.text_edit.any_active() {
                    self.ui_context.text_edit.set_text(text);
                }
            }
            geng::Event::KeyPress {
                key: geng::Key::F11,
            } => {
                self.context.geng.window().toggle_fullscreen();
            }
            _ => (),
        }
    }

    fn transition(&mut self) -> Option<geng::state::Transition> {
        if let Some(action) = self.state.action.take() {
            let state: Box<dyn geng::State> = match action {
                Action::CreateRoom => {
                    log::info!("Creating a room...");
                    let context = self.context.clone();
                    let connect = self.connect.clone();
                    let test = self.test;
                    let future = async move {
                        let mut connection =
                            crate::interop::ClientConnection::connect(&connect.unwrap())
                                .await
                                .unwrap();
                        connection.send(ClientMessage::CreateRoom);
                        let mut new_token = None;
                        let room_info = loop {
                            let message = connection.next().await.unwrap().unwrap();
                            match message {
                                ServerMessage::Ping => connection.send(ClientMessage::Pong),
                                ServerMessage::YourToken(token) => new_token = Some(token),
                                ServerMessage::RoomJoined(room_info) => break room_info,
                                ServerMessage::Error(error) => {
                                    log::error!("Failed to create a room: {error}");
                                    return None;
                                }
                                _ => {
                                    log::error!("Failed to create a room");
                                    return None;
                                }
                            }
                        };

                        if let Some(token) = preferences::load("usertoken") {
                            connection.send(ClientMessage::Login(token));
                        } else if let Some(token) = new_token {
                            preferences::save("usertoken", &token);
                        }

                        Some(
                            crate::menu::lobby::Lobby::new(&context, connection, room_info, test)
                                .await,
                        )
                    }
                    .boxed_local();
                    Box::new(LoadingScreen::new(
                        &self.context.geng,
                        geng::EmptyLoadingScreen::new(&self.context.geng),
                        future,
                    ))
                }
                Action::Join(code) => {
                    log::info!("Joining room {code}...");
                    let context = self.context.clone();
                    let connect = self.connect.clone();
                    let test = self.test;
                    let future = async move {
                        let mut connection =
                            crate::interop::ClientConnection::connect(&connect.unwrap())
                                .await
                                .unwrap();

                        if let Some(token) = preferences::load("usertoken") {
                            connection.send(ClientMessage::Login(token));
                        }

                        connection.send(ClientMessage::JoinRoom(code));
                        let mut new_token = None;
                        let room_info = loop {
                            let message = connection.next().await.unwrap().unwrap();
                            match message {
                                ServerMessage::Ping => connection.send(ClientMessage::Pong),
                                ServerMessage::YourToken(token) => new_token = Some(token),
                                ServerMessage::RoomJoined(room_info) => break room_info,
                                ServerMessage::Error(error) => {
                                    log::error!("Failed to join the room: {error}");
                                    return None;
                                }
                                _ => {
                                    log::error!("Failed to join the room");
                                    return None;
                                }
                            }
                        };

                        if let Some(token) = new_token {
                            preferences::save("usertoken", &token);
                        }

                        Some(
                            crate::menu::lobby::Lobby::new(&context, connection, room_info, test)
                                .await,
                        )
                    }
                    .boxed_local();
                    Box::new(LoadingScreen::new(
                        &self.context.geng,
                        geng::EmptyLoadingScreen::new(&self.context.geng),
                        future,
                    ))
                }
            };
            self.context.geng.window().stop_text_edit();
            return Some(geng::state::Transition::Push(state));
        }

        None
    }

    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::WHITE), Some(1.0), None);

        self.ui_context.state.frame_start();
        self.ui_context.geometry.update(framebuffer.size());

        self.ui.layout(
            &mut self.state,
            Aabb2::ZERO.extend_positive(framebuffer.size().as_f32()),
            &mut self.ui_context,
        );
        self.ui_context.frame_end();

        let geometry = RefCell::new(Geometry::new());
        self.ui_context.state.iter_widgets(
            |w| {
                geometry.borrow_mut().merge(w.draw_top(&self.ui_context));
            },
            |w| {
                geometry.borrow_mut().merge(w.draw(&self.ui_context));
            },
        );
        let geometry = geometry.into_inner();

        self.util_render.draw_geometry(
            &mut self.mask_stack,
            geometry,
            &geng::PixelPerfectCamera,
            framebuffer,
        );
    }
}

impl MainMenuUi {
    pub fn new() -> Self {
        Self {}
    }

    pub fn layout(
        &mut self,
        state: &mut MainMenuState,
        screen: Aabb2<f32>,
        context: &mut UiContext,
    ) {
        let screen = screen.fit_aabb(vec2(16.0, 9.0), vec2(0.5, 0.5));
        context.screen = screen;
        context.font_size = screen.height() * 0.05;
        context.layout_size = screen.height() * 0.07;

        let screen_ratio = screen.size() / vec2(1920.0, 1080.0);

        let assets = context.context.assets.get();
        let atlas = &assets.atlas;

        context
            .state
            .get_root_or(|| IconWidget::new(atlas.menu()))
            .update(screen, context);

        let mk_button = |pos: vec2<f32>, size| {
            Aabb2::point(screen.align_pos(vec2(pos.x, 1080.0 - pos.y) / vec2(1920.0, 1080.0)))
                .extend_symmetric(size * screen_ratio / 2.0)
        };

        let mut create = mk_button(vec2(309.0, 453.0), vec2(483.0, 118.0));
        let button = context.state.get_root_or(|| {
            ButtonWidget::new(atlas.button_background()).with_text("Создать комнату")
        });
        button.text.options.color = assets.palette.text;
        if create.contains(context.cursor.position) {
            create = create.extend_symmetric(
                vec2(atlas.button_background().size().as_f32().aspect(), 1.0) * 10.0,
            );
        }
        button.update(create, context);
        if button.state.mouse_left.clicked {
            state.action = Some(Action::CreateRoom);
        }

        let mut join = mk_button(vec2(320.0, 620.0), vec2(483.0, 118.0));
        let join_button = context.state.get_root_or(|| {
            ButtonWidget::new(atlas.button_background()).with_text("Присоединиться")
        });
        join_button.text.options.color = assets.palette.text;
        if join.contains(context.cursor.position) {
            join = join.extend_symmetric(
                vec2(atlas.button_background().size().as_f32().aspect(), 1.0) * 10.0,
            );
        }
        join_button.update(join, context);

        let mut code = mk_button(vec2(715.0, 625.0), vec2(210.0, 80.0));
        if code.contains(context.cursor.position) {
            code = code.extend_symmetric(
                vec2(atlas.code_background().size().as_f32().aspect(), 1.0) * 10.0,
            );
        }
        let code_input = context
            .state
            .get_root_or(|| InputWidget::new("").max_len(4).uppercase());
        code_input.update(code, context);
        code_input.name.options.color = assets.palette.text;
        context
            .state
            .get_root_or(|| IconWidget::new(atlas.code_background()))
            .update(
                code.extend_symmetric(
                    vec2(atlas.code_background().size().as_f32().aspect(), 1.0) * 2.0,
                ),
                context,
            );

        if join_button.state.mouse_left.clicked {
            state.action = Some(Action::Join(code_input.raw.clone()));
        }
    }
}
