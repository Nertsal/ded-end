mod logic;
mod player;
mod render;

use player::*;

use super::*;

use crate::{
    assets::{SolverItem, SolverItemKind},
    interop::{ClientConnection, ClientMessage, ServerMessage},
    model::*,
    ui::layout::AreaOps,
};

use geng_utils::conversions::*;

const SCREEN_SIZE: vec2<usize> = vec2(1920, 1080);
const LEVEL_SIZE: vec2<f32> = vec2(16.0, 9.0);

pub struct GameSolver {
    context: Context,
    connection: ClientConnection,
    test: bool,

    final_texture: ugli::Texture,
    framebuffer_size: vec2<usize>,
    screen: Aabb2<f32>,

    client_state: SolverStateClient,
    state: SolverState,
    dispatcher_state: DispatcherState,
    camera: Camera2d,

    player_control: PlayerControl,
}

struct SolverStateClient {
    time: FTime,
    dedend: Option<(Option<FTime>, String)>,
    player: Player,
    level_static_colliders: Vec<Collider>,
    door_entrance: Collider,
    door_exit: Collider,
    platforms: Vec<Collider>,
    bubble_balls: Vec<(Collider, usize)>,
    items: Vec<SolverItem>,
    picked_up_item: Option<SolverItem>,
    explosion: Option<(vec2<FCoord>, FTime)>,
    grandson_spin: Option<Angle<FCoord>>,
    grandpa_drill: Option<FTime>,
    bubble_code: Vec<usize>,
    interact_item: Option<usize>,
    projectiles: Vec<Projectile>,
    fish_cooldown: FTime,
}

impl SolverStateClient {
    fn new() -> Self {
        Self {
            time: FTime::ZERO,
            dedend: None,
            player: Player {
                collider: Collider::aabb(
                    Aabb2::point(vec2(0.0, 0.0))
                        .extend_positive(vec2(1.0, 1.5))
                        .as_r32(),
                ),
                velocity: vec2::ZERO,
                state: PlayerState::Airborn,
                control_timeout: None,
                facing_left: false,
                can_hold_jump: false,
                coyote_time: None,
                jump_buffer: None,
                animation_time: FTime::ZERO,
            },
            level_static_colliders: Vec::new(),
            door_entrance: Collider::aabb(Aabb2::ZERO),
            door_exit: Collider::aabb(Aabb2::ZERO),
            platforms: Vec::new(),
            bubble_balls: Vec::new(),
            items: Vec::new(),
            picked_up_item: None,
            explosion: None,
            grandson_spin: None,
            grandpa_drill: None,
            bubble_code: Vec::new(),
            interact_item: None,
            projectiles: Vec::new(),
            fish_cooldown: FTime::new(1.0),
        }
    }
}

struct Projectile {
    pub collider: Collider,
    pub velocity: vec2<FCoord>,
}

impl GameSolver {
    pub fn new(context: &Context, connection: ClientConnection, test: Option<usize>) -> Self {
        let assets = context.assets.get();
        context.music.play_music(&assets.sounds.dispatcher);

        let mut game = Self {
            context: context.clone(),
            connection,
            test: test.is_some(),

            final_texture: geng_utils::texture::new_texture(context.geng.ugli(), SCREEN_SIZE),
            framebuffer_size: vec2(1, 1),
            screen: Aabb2::ZERO.extend_positive(vec2(1.0, 1.0)),

            client_state: SolverStateClient::new(),
            state: SolverState::new(),
            dispatcher_state: DispatcherState::new(),
            camera: Camera2d {
                center: LEVEL_SIZE / 2.0,
                rotation: Angle::ZERO,
                fov: Camera2dFov::Cover {
                    width: LEVEL_SIZE.x,
                    height: LEVEL_SIZE.y,
                    scale: 1.0,
                },
            },

            player_control: PlayerControl::default(),
        };

        if let Some(test) = test {
            game.state.current_level = test;
            game.state.levels_completed = test;
            game.connection
                .send(ClientMessage::SyncSolverState(game.state.clone()));
        }

        game.reload_level();
        game
    }

    fn reload_level(&mut self) {
        if self.state.current_level == 4 {
            self.context
                .music
                .play_music(&self.context.assets.get().sounds.boss);
        } else if self.state.current_level > 4 {
            self.context
                .music
                .play_music(&self.context.assets.get().sounds.dispatcher);
        }

        self.client_state = SolverStateClient::new();

        self.player_respawn();
        self.update_level_colliders();
        self.reset_items();
    }

    fn reset_items(&mut self) {
        let assets = self.context.assets.get();
        let Some(level) = assets.solver.levels.get(self.state.current_level) else {
            return;
        };
        self.client_state.items = level.items.clone();
        self.client_state.picked_up_item = None;
    }

    fn update_level_colliders(&mut self) {
        let assets = self.context.assets.get();
        let Some(level) = assets.solver.levels.get(self.state.current_level) else {
            return;
        };

        let wall_thickness = r32(1.0);
        let door_height = r32(2.0);

        // Floor
        self.client_state
            .level_static_colliders
            .push(Collider::aabb(
                Aabb2::ZERO
                    .extend_right(r32(LEVEL_SIZE.x))
                    .extend_up(wall_thickness),
            ));
        // Left wall
        self.client_state
            .level_static_colliders
            .push(Collider::aabb(
                Aabb2::point(vec2(r32(0.0), door_height + wall_thickness))
                    .extend_up(r32(LEVEL_SIZE.y))
                    .extend_right(wall_thickness),
            ));
        // Right wall
        self.client_state
            .level_static_colliders
            .push(Collider::aabb(
                Aabb2::point(vec2(LEVEL_SIZE.x.as_r32(), door_height + wall_thickness))
                    .extend_up(r32(LEVEL_SIZE.y))
                    .extend_left(wall_thickness),
            ));
        // Ceiling
        self.client_state
            .level_static_colliders
            .push(Collider::aabb(
                Aabb2::point(vec2(0.0, LEVEL_SIZE.y).as_r32())
                    .extend_right(r32(LEVEL_SIZE.x))
                    .extend_down(wall_thickness),
            ));

        let door_width = r32(0.3);

        // Entrance door
        self.client_state.door_entrance = Collider::aabb(
            Aabb2::point(vec2(0.0.as_r32(), wall_thickness))
                .extend_up(door_height)
                .extend_right(door_width),
        );

        // Exit door
        self.client_state.door_exit = Collider::aabb(
            Aabb2::point(vec2(LEVEL_SIZE.x.as_r32(), wall_thickness))
                .extend_up(door_height)
                .extend_left(door_width),
        );

        // Platforms
        let platform_size = assets.solver.sprites.platform.size().as_f32();
        self.client_state.platforms = level
            .platforms
            .iter()
            .map(|platform| {
                let size = vec2(
                    platform.width,
                    platform.width / platform_size.aspect().as_r32(),
                );
                Collider::aabb(
                    Aabb2::point(platform.pos)
                        .extend_symmetric(vec2(size.x, r32(0.0) / r32(2.0)))
                        .extend_down(size.y),
                )
            })
            .collect();

        if self.state.current_level == 3 {
            // Bubble tea walls
            self.client_state.level_static_colliders.push(Collider {
                position: vec2(5.0, 4.5).as_r32(),
                rotation: Angle::from_degrees(15.0).as_r32(),
                shape: Shape::Rectangle {
                    width: r32(0.07),
                    height: r32(9.0),
                },
            });
            self.client_state.level_static_colliders.push(Collider {
                position: vec2(11.0, 9.0 - 3.3).as_r32(),
                rotation: Angle::from_degrees(-15.0).as_r32(),
                shape: Shape::Rectangle {
                    width: r32(0.07),
                    height: r32(4.5),
                },
            });
            // Door
            self.client_state.level_static_colliders.push(Collider {
                position: vec2(10.15, 9.0 - 7.05).as_r32(),
                rotation: Angle::from_degrees(-15.0).as_r32(),
                shape: Shape::Rectangle {
                    width: r32(0.07),
                    height: r32(2.4),
                },
            });

            // Bubbles
            let ball = &Collider::circle(vec2::ZERO, r32(0.5));
            self.client_state.bubble_balls = (0..4)
                .flat_map(|x| {
                    (0..6).map(move |y| {
                        let mut rng = thread_rng();
                        let mut ball = ball.clone();
                        ball.position = vec2(6.5, 1.5).as_r32()
                            + vec2(x, y).as_r32()
                            + vec2(rng.gen_range(-0.01..=0.01), rng.gen_range(-0.01..=0.01))
                                .as_r32();
                        (ball, rng.gen_range(0..=3))
                    })
                })
                .collect();
        }
    }

    fn handle_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Ping
            | ServerMessage::RoomJoined(..)
            | ServerMessage::StartGame(..)
            | ServerMessage::SyncSolverPlayer(_)
            | ServerMessage::YourToken(_)
            | ServerMessage::SyncRoomPlayers(_) => {}
            ServerMessage::Error(error) => log::error!("Server error: {error}"),
            ServerMessage::SyncDispatcherState(dispatcher_state) => {
                self.dispatcher_state = dispatcher_state
            }
            ServerMessage::SyncSolverState(solver_state) => self.state = solver_state,
            ServerMessage::GameCrash(auto_reboot, message) => self.game_crash(auto_reboot, message),
        }
    }

    fn game_crash(&mut self, auto_reboot: bool, message: impl Into<String>) {
        let message = message.into();
        log::info!("DED END: {message}");
        self.client_state.dedend = Some((auto_reboot.then(|| FTime::new(5.0)), message));
    }

    fn submit_bubble_code(&mut self) {
        if !self.state.solved_bubble_code && self.client_state.bubble_code == vec![4, 2, 1, 3] {
            self.context.geng.window().stop_text_edit();
            self.state.solved_bubble_code = true;
            self.client_state.level_static_colliders.pop();
            self.connection
                .send(ClientMessage::SyncSolverState(self.state.clone()));
        }
    }

    fn press_enter(&mut self) {
        if self.state.current_level == 3 {
            self.submit_bubble_code();
        }
    }

    fn press_escape(&mut self) {}

    fn press_backspace(&mut self) {
        if self.state.current_level == 3 && !self.state.solved_bubble_code {
            self.client_state.bubble_code.pop();
        }
    }

    fn press_digit(&mut self, digit: usize) {
        if self.state.current_level == 3
            && !self.state.solved_bubble_code
            && self.client_state.bubble_code.len() < 4
        {
            self.client_state.bubble_code.push(digit);
        }
    }
}

impl geng::State for GameSolver {
    fn update(&mut self, delta_time: f64) {
        self.update(FTime::new(delta_time as f32));
    }

    fn handle_event(&mut self, event: geng::Event) {
        let assets = self.context.assets.get();
        let controls = &assets.solver.controls;
        if geng_utils::key::is_event_press(&event, &controls.jump) {
            self.player_control.jump = true;
        }

        if geng_utils::key::is_event_press(&event, &controls.pickup) {
            self.player_control.pickup = true;
        }

        drop(assets);
        if let geng::Event::KeyPress { key } = event {
            match key {
                geng::Key::F11 => {
                    self.context.geng.window().toggle_fullscreen();
                }
                geng::Key::F5 => {
                    self.reload_level();
                }
                geng::Key::F4 if self.test => {
                    // Prev level
                    self.state.current_level = self.state.current_level.saturating_sub(1);
                    self.state.levels_completed = self.state.current_level;
                    self.reload_level();
                }
                geng::Key::F6 if self.test => {
                    // Next level
                    self.state.current_level += 1;
                    self.state.levels_completed = self.state.current_level;
                    self.reload_level();
                }
                geng::Key::Escape => self.press_escape(),
                geng::Key::Backspace => self.press_backspace(),
                geng::Key::Enter => self.press_enter(),
                geng::Key::Digit0 => self.press_digit(0),
                geng::Key::Digit1 => self.press_digit(1),
                geng::Key::Digit2 => self.press_digit(2),
                geng::Key::Digit3 => self.press_digit(3),
                geng::Key::Digit4 => self.press_digit(4),
                geng::Key::Digit5 => self.press_digit(5),
                geng::Key::Digit6 => self.press_digit(6),
                geng::Key::Digit7 => self.press_digit(7),
                geng::Key::Digit8 => self.press_digit(8),
                geng::Key::Digit9 => self.press_digit(9),
                _ => {}
            }
        }
    }

    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size();
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);
        self.draw_game();
        let draw = geng_utils::texture::DrawTexture::new(&self.final_texture)
            .fit_screen(vec2(0.5, 0.5), framebuffer);
        self.screen = draw.target;
        draw.draw(&geng::PixelPerfectCamera, &self.context.geng, framebuffer);
    }
}
