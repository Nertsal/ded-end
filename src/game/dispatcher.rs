use super::*;

use crate::{
    assets::*,
    interop::{ClientConnection, ClientMessage, ServerMessage},
    model::{DispatcherState, FTime, Player, PlayerAnimationState, SolverState},
    ui::layout::AreaOps,
};

use geng_utils::{
    conversions::{Aabb2RealConversions, Vec2RealConversions},
    interpolation::SecondOrderState,
};

const SCREEN_SIZE: vec2<usize> = vec2(1920, 1080);

pub struct GameDispatcher {
    context: Context,
    connection: ClientConnection,

    final_texture: ugli::Texture,
    framebuffer_size: vec2<usize>,
    screen: Aabb2<f32>,
    /// Default scaling from texture to SCREEN_SIZE.
    texture_scaling: f32,
    camera: Camera2d,
    camera_fov: SecondOrderState<f32>,
    camera_center: SecondOrderState<vec2<f32>>,
    solver_camera: Camera2d,
    time: FTime,

    cursor_position_raw: vec2<f64>,
    cursor_position_game: vec2<f32>,

    client_state: DispatcherStateClient,
    state: DispatcherState,
    solver_state: SolverState,
    solver_player: Option<Player>,
    ui: DispatcherUi,
}

struct DispatcherUi {
    items_layout: HashMap<(DispatcherViewSide, usize), Aabb2<f32>>,
    monitor: Aabb2<f32>,
    monitor_inside: Aabb2<f32>,
    login_code: Vec<Aabb2<f32>>,
    user_icon: Aabb2<f32>,
    files: Vec<Aabb2<f32>>,
    meme_folder: Option<Aabb2<f32>>,
    meme_prev: Aabb2<f32>,
    meme_next: Aabb2<f32>,
    opened_file: Aabb2<f32>,

    button_station_inside: Aabb2<f32>,

    turn_left: Aabb2<f32>,
    turn_right: Aabb2<f32>,
}

pub struct DispatcherStateClient {
    hovering_smth: bool,
    active_side: DispatcherViewSide,
    focus: Focus,
    login_code: Vec<usize>,
    opened_file: Option<usize>,
    opened_meme: Option<usize>,
    bfb_pressed: Option<FTime>,
    buttons_pressed: HashMap<DispatcherItem, FTime>,
    bubble_buttons: usize,
    explosion: Option<(vec2<f32>, FTime)>,
    novella: Option<NovellaState>,
}

struct NovellaState {
    sprite: Rc<PixelTexture>,
    line: usize,
    character: usize,
    fast: bool,
    next_char_in: f32,
    is_line_done: bool,
}

impl NovellaState {
    pub fn new(assets: &Assets) -> Self {
        Self {
            sprite: assets.dispatcher.sprites.novella.neutral.clone(),
            line: 0,
            character: 0,
            fast: false,
            next_char_in: 1.0,
            is_line_done: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Whole,
    Monitor,
    Book,
}

impl GameDispatcher {
    pub fn new(context: &Context, connection: ClientConnection, test: Option<usize>) -> Self {
        let assets = context.assets.get();
        context.music.play_music(&assets.sounds.dispatcher);

        const TURN_BUTTON_SIZE: vec2<f32> = vec2(50.0, 50.0);
        let mut game = Self {
            context: context.clone(),
            connection,

            final_texture: geng_utils::texture::new_texture(context.geng.ugli(), SCREEN_SIZE),
            framebuffer_size: vec2(1, 1),
            screen: Aabb2::ZERO.extend_positive(vec2(1.0, 1.0)),
            texture_scaling: 1.0,
            camera: Camera2d {
                center: SCREEN_SIZE.as_f32() / 2.0,
                rotation: Angle::ZERO,
                fov: Camera2dFov::Vertical(SCREEN_SIZE.y as f32),
            },
            camera_fov: SecondOrderState::new(1.5, 1.0, 0.0, SCREEN_SIZE.y as f32),
            camera_center: SecondOrderState::new(1.5, 1.0, 0.0, SCREEN_SIZE.as_f32() / 2.0),
            solver_camera: Camera2d {
                center: vec2(8.0, 4.5),
                rotation: Angle::ZERO,
                fov: Camera2dFov::Cover {
                    width: 16.0,
                    height: 9.0,
                    scale: 1.0,
                },
            },
            time: FTime::ZERO,

            cursor_position_raw: vec2::ZERO,
            cursor_position_game: vec2::ZERO,

            client_state: DispatcherStateClient {
                hovering_smth: false,
                active_side: DispatcherViewSide::Back,
                focus: Focus::Whole,
                login_code: vec![],
                opened_file: None,
                opened_meme: None,
                bfb_pressed: None,
                buttons_pressed: HashMap::new(),
                bubble_buttons: 0,
                explosion: None,
                novella: None,
            },
            state: DispatcherState::new(),
            solver_state: SolverState::new(),
            solver_player: None,
            ui: DispatcherUi {
                items_layout: HashMap::new(),
                monitor: Aabb2::ZERO,
                monitor_inside: Aabb2::ZERO,
                login_code: vec![],
                user_icon: Aabb2::ZERO,
                files: vec![],
                meme_folder: None,
                meme_prev: Aabb2::ZERO,
                meme_next: Aabb2::ZERO,
                opened_file: Aabb2::ZERO,

                button_station_inside: Aabb2::ZERO,

                turn_left: Aabb2::point(vec2(TURN_BUTTON_SIZE.x / 2.0, SCREEN_SIZE.y as f32 / 2.0))
                    .extend_symmetric(TURN_BUTTON_SIZE / 2.0),
                turn_right: Aabb2::point(vec2(
                    SCREEN_SIZE.x as f32 - TURN_BUTTON_SIZE.x / 2.0,
                    SCREEN_SIZE.y as f32 / 2.0,
                ))
                .extend_symmetric(TURN_BUTTON_SIZE / 2.0),
            },
        };
        if let Some(test) = test {
            game.solver_state.current_level = test;
            game.solver_state.levels_completed = test;
            game.connection
                .send(ClientMessage::SyncSolverState(game.solver_state.clone()));
        }
        game
    }

    fn draw_game(&mut self) {
        let assets = self.context.assets.get();
        let framebuffer = &mut geng_utils::texture::attach_texture(
            &mut self.final_texture,
            self.context.geng.ugli(),
        );
        ugli::clear(framebuffer, Some(assets.palette.background), None, None);

        if let Some(novella) = &self.client_state.novella {
            let camera = Camera2d {
                center: SCREEN_SIZE.as_f32() / 2.0,
                rotation: Angle::ZERO,
                fov: Camera2dFov::Vertical(SCREEN_SIZE.y as f32),
            };
            let sprites = &assets.dispatcher.sprites.novella;
            let text = &assets.dispatcher.novella;

            let screen = Aabb2::ZERO.extend_positive(SCREEN_SIZE.as_f32());

            geng_utils::texture::DrawTexture::new(&sprites.background)
                .fit_height(screen, 0.5)
                .draw(&camera, &self.context.geng, framebuffer);
            geng_utils::texture::DrawTexture::new(&novella.sprite)
                .fit(screen, vec2(0.5, 0.0))
                .draw(&camera, &self.context.geng, framebuffer);

            let textbox = screen
                .align_aabb(vec2(750.0, 375.0), vec2(0.5, 0.0))
                .translate(vec2(0.0, 50.0));
            geng_utils::texture::DrawTexture::new(&sprites.textbox)
                .fit_height(textbox, 0.5)
                .draw(&camera, &self.context.geng, framebuffer);

            if let Some(line) = text.lines().nth(novella.line) {
                let line: String = line.chars().take(novella.character).collect();
                draw_text(
                    &assets.font,
                    &line,
                    100.0,
                    assets.palette.text,
                    textbox.extend_uniform(-10.0),
                    &camera,
                    framebuffer,
                );
            }

            return;
        }

        self.client_state.hovering_smth = false;

        let sprites = &assets.dispatcher.sprites;

        let level = assets
            .dispatcher
            .level
            .get_side(self.client_state.active_side);
        let mut draw_monitor = false;
        for (item_index, (item, positioning)) in level.items.iter().enumerate() {
            let mut color = Rgba::<f32>::WHITE;

            macro_rules! button {
                ($color:literal) => {{
                    if !self.state.button_station_open {
                        continue;
                    }
                    color = Rgba::try_from($color).unwrap();
                    if self.client_state.buttons_pressed.contains_key(item) {
                        &sprites.button_pressed
                    } else {
                        &sprites.button
                    }
                }};
            }

            let texture = match item {
                DispatcherItem::Door => &sprites.door,
                DispatcherItem::DoorSign => {
                    if self.state.door_sign_open {
                        &sprites.sign_open
                    } else {
                        &sprites.sign_closed
                    }
                }
                DispatcherItem::Table => &sprites.table,
                DispatcherItem::Monitor => &sprites.monitor,
                DispatcherItem::RealMouse => &sprites.real_mouse,
                DispatcherItem::Cactus => &sprites.cactus,
                DispatcherItem::Book => &sprites.book,
                DispatcherItem::TheSock => &sprites.the_sock,
                DispatcherItem::ButtonStation => {
                    if self.state.button_station_open {
                        &sprites.button_station_open
                    } else {
                        &sprites.button_station_closed
                    }
                }
                DispatcherItem::Bfb => {
                    if self.client_state.bfb_pressed.is_some() {
                        &sprites.button_big_pressed
                    } else {
                        &sprites.button_big
                    }
                }
                DispatcherItem::ButtonYellow => button!("#FFFF00"),
                DispatcherItem::ButtonGreen => button!("#00FF00"),
                DispatcherItem::ButtonSalad => button!("#ECFF00"),
                DispatcherItem::ButtonPink => button!("#FFC0CB"),
                DispatcherItem::ButtonBlue => button!("#0022EE"),
                DispatcherItem::ButtonWhite => button!("#FFFFFF"),
                DispatcherItem::ButtonPurple => button!("#800080"),
                DispatcherItem::ButtonOrange => button!("#FFA500"),
                DispatcherItem::ButtonCyan => button!("#00EEEE"),
                DispatcherItem::Tea => &sprites.tea,
            };
            let size = positioning
                .size
                .unwrap_or(texture.size().as_f32() * self.texture_scaling);
            let pos = Aabb2::point(positioning.anchor - size * positioning.alignment)
                .extend_positive(size);
            let mut draw = geng_utils::texture::DrawTexture::new(texture)
                .colored(color)
                .fit(pos, vec2(0.5, 0.5));

            self.ui
                .items_layout
                .insert((self.client_state.active_side, item_index), draw.target);
            if let DispatcherItem::Monitor = item {
                draw_monitor = true;
                self.ui.monitor = draw.target;
                self.ui.monitor_inside = Aabb2::from_corners(
                    vec2(27.0, -32.0) / vec2(549.0, 513.0) * self.ui.monitor.size(),
                    vec2(519.0, -311.0) / vec2(549.0, 513.0) * self.ui.monitor.size(),
                )
                .translate(self.ui.monitor.top_left())
                .fit_aabb(sprites.login_screen.size().as_f32(), vec2(0.5, 0.5));

                let convert = |pos: vec2<usize>| -> vec2<f32> {
                    pos.as_f32() * vec2(1.0, -1.0) / vec2(1045.0, 685.0)
                        * self.ui.monitor_inside.size()
                };
                self.ui.user_icon = Aabb2::point(convert(vec2(520, 320)))
                    .extend_uniform(30.0)
                    .translate(self.ui.monitor_inside.top_left());

                let digit = |a: vec2<usize>, b: vec2<usize>| {
                    Aabb2::from_corners(convert(a), convert(b))
                        .translate(self.ui.monitor_inside.top_left())
                };
                self.ui.login_code = vec![
                    digit(vec2(445, 440), vec2(482, 487)),
                    digit(vec2(518, 432), vec2(561, 484)),
                    digit(vec2(586, 435), vec2(618, 482)),
                ];

                let convert = |pos: vec2<usize>| -> vec2<f32> {
                    pos.as_f32() * vec2(1.0, -1.0) / vec2(1039.0, 665.0)
                        * self.ui.monitor_inside.size()
                };

                let file_size = vec2(30.0, 20.0);
                let file = |pos: vec2<usize>| {
                    Aabb2::point(convert(pos))
                        .extend_symmetric(file_size / 2.0)
                        .translate(self.ui.monitor_inside.top_left())
                };
                self.ui.files = vec![
                    file(vec2(135, 465)),
                    file(vec2(205, 465)),
                    file(vec2(275, 465)),
                    file(vec2(345, 465)),
                    file(vec2(415, 465)),
                ];

                if self.solver_state.current_level >= 2 {
                    self.ui.meme_folder = Some(file(vec2(680, 540)).extend_uniform(15.0));
                }

                self.ui.opened_file =
                    Aabb2::from_corners(convert(vec2(423, 26)), convert(vec2(981, 578)))
                        .translate(self.ui.monitor_inside.top_left());
                self.ui.meme_prev = file(vec2(470, 260));
                self.ui.meme_next = file(vec2(930, 260));
            }

            if let DispatcherItem::ButtonStation = item {
                let size = sprites.button_station_closed.size().as_f32();
                self.ui.button_station_inside = draw
                    .target
                    .align_aabb(size, vec2(1.0, 1.0))
                    .extend_uniform(-size.x * 0.1);
            }

            if item.is_interactable()
                && draw.target.contains(self.cursor_position_game)
                && !(*item == DispatcherItem::Monitor && self.client_state.focus == Focus::Monitor)
                && !(*item == DispatcherItem::ButtonStation
                    && self.state.button_station_open
                    && self
                        .ui
                        .button_station_inside
                        .contains(self.cursor_position_game))
                && !(*item == DispatcherItem::Bfb && self.client_state.bfb_pressed.is_some())
                && !self.client_state.buttons_pressed.contains_key(item)
            {
                self.client_state.hovering_smth = true;
                draw.target = draw.target.extend_uniform(10.0);
            }

            if let DispatcherItem::ButtonYellow
            | DispatcherItem::ButtonGreen
            | DispatcherItem::ButtonSalad
            | DispatcherItem::ButtonPink
            | DispatcherItem::ButtonBlue
            | DispatcherItem::ButtonWhite
            | DispatcherItem::ButtonPurple
            | DispatcherItem::ButtonOrange
            | DispatcherItem::ButtonCyan = item
            {
                let mut draw_base = geng_utils::texture::DrawTexture::new(&sprites.button_base);
                draw_base.target = draw.target;
                draw_base.draw(&self.camera, &self.context.geng, framebuffer);
            }

            draw.draw(&self.camera, &self.context.geng, framebuffer);
        }

        for (texture, mut target) in [
            (&sprites.arrow_left, self.ui.turn_left),
            (&sprites.arrow_right, self.ui.turn_right),
        ] {
            if target.contains(self.cursor_position_game) {
                self.client_state.hovering_smth = true;
                target = target.extend_uniform(target.width() * 0.1);
            }
            geng_utils::texture::DrawTexture::new(texture)
                .fit(target, vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        let monitor_focused = self.client_state.focus == Focus::Monitor;
        if draw_monitor {
            // Monitor
            if self.state.monitor_unlocked {
                // Workspace
                geng_utils::texture::DrawTexture::new(&sprites.workspace)
                    .fit(self.ui.monitor_inside, vec2(0.5, 0.5))
                    .draw(&self.camera, &self.context.geng, framebuffer);

                // Files
                for pos in self
                    .ui
                    .files
                    .iter()
                    .take(self.solver_state.levels_completed + 1)
                {
                    let mut pos = *pos;
                    if monitor_focused && pos.contains(self.cursor_position_game) {
                        self.client_state.hovering_smth = true;
                        pos = pos.extend_uniform(3.0);
                    }
                    geng_utils::texture::DrawTexture::new(&sprites.file)
                        .fit(pos, vec2(0.5, 0.5))
                        .draw(&self.camera, &self.context.geng, framebuffer);
                }

                if let Some(mut pos) = self.ui.meme_folder {
                    if monitor_focused && pos.contains(self.cursor_position_game) {
                        self.client_state.hovering_smth = true;
                        pos = pos.extend_uniform(3.0);
                    }
                    geng_utils::texture::DrawTexture::new(&sprites.meme_folder)
                        .fit(pos, vec2(0.5, 0.5))
                        .draw(&self.camera, &self.context.geng, framebuffer);
                }

                // Opened file
                if let Some(file) = self.client_state.opened_file {
                    let draw = geng_utils::texture::DrawTexture::new(&sprites.file_window)
                        .fit(self.ui.opened_file, vec2(0.5, 0.5));
                    let window = draw.target;
                    draw.draw(&self.camera, &self.context.geng, framebuffer);

                    if let Some(text) = assets.dispatcher.files.get(file) {
                        draw_text(
                            &assets.font,
                            text,
                            10.0,
                            assets.palette.text,
                            window.extend_uniform(-30.0),
                            &self.camera,
                            framebuffer,
                        );
                    }
                } else if let Some(i) = self.client_state.opened_meme {
                    let draw = geng_utils::texture::DrawTexture::new(&sprites.file_window)
                        .fit(self.ui.opened_file, vec2(0.5, 0.5));
                    let window = draw.target;
                    draw.draw(&self.camera, &self.context.geng, framebuffer);

                    if let Some(texture) = sprites.memes.get(i) {
                        geng_utils::texture::DrawTexture::new(texture)
                            .fit(window.extend_uniform(-50.0), vec2(0.5, 0.5))
                            .draw(&self.camera, &self.context.geng, framebuffer);
                    }
                    let mut prev = self.ui.meme_prev;
                    if prev.contains(self.cursor_position_game) {
                        self.client_state.hovering_smth = true;
                        prev = prev.extend_uniform(3.0);
                    }
                    geng_utils::texture::DrawTexture::new(&sprites.arrow_left)
                        .fit(prev, vec2(0.5, 0.5))
                        .draw(&self.camera, &self.context.geng, framebuffer);
                    let mut next = self.ui.meme_next;
                    if next.contains(self.cursor_position_game) {
                        next = next.extend_uniform(3.0);
                    }
                    geng_utils::texture::DrawTexture::new(&sprites.arrow_right)
                        .fit(next, vec2(0.5, 0.5))
                        .draw(&self.camera, &self.context.geng, framebuffer);
                }
            } else {
                // Login
                geng_utils::texture::DrawTexture::new(&sprites.login_screen)
                    .fit(self.ui.monitor_inside, vec2(0.5, 0.5))
                    .draw(&self.camera, &self.context.geng, framebuffer);

                // Profile
                let mut user_icon = self.ui.user_icon;
                if monitor_focused && user_icon.contains(self.cursor_position_game) {
                    self.client_state.hovering_smth = true;
                    user_icon = user_icon.extend_uniform(5.0);
                }
                geng_utils::texture::DrawTexture::new(&sprites.user_icon)
                    .fit(user_icon, vec2(0.5, 0.5))
                    .draw(&self.camera, &self.context.geng, framebuffer);

                // Code
                let font = self.context.geng.default_font();
                for (digit, pos) in self.client_state.login_code.iter().zip(&self.ui.login_code) {
                    self.context.geng.draw2d().draw2d(
                        framebuffer,
                        &self.camera,
                        &draw2d::Text::unit(&**font, digit.to_string(), assets.palette.text)
                            .fit_into(*pos),
                    );
                }
            }
        }

        // Player
        if !self.solver_state.popped
            && let Some(player) = &self.solver_player
            && let DispatcherViewSide::Front = self.client_state.active_side
        {
            let animation = |frames: &[Rc<crate::assets::PixelTexture>], frame_time: f32| {
                let frame_time = r32(frame_time);
                let frame = (player.animation_time / frame_time)
                    .as_f32()
                    .max(0.0)
                    .floor() as usize
                    % frames.len();
                frames[frame].clone()
            };
            let texture = match player.animation_state() {
                PlayerAnimationState::Idle => animation(&assets.solver.sprites.player.idle, 0.5),
                PlayerAnimationState::Running => {
                    animation(&assets.solver.sprites.player.running, 0.1)
                }
                PlayerAnimationState::Jumping => {
                    let sprites = &assets.solver.sprites.player.jump;
                    if player.velocity.y.as_f32() > 0.0 {
                        sprites[0].clone()
                    } else {
                        sprites[1].clone()
                    }
                }
            };
            let flip = !player.facing_left;

            let mut pos = player.collider.compute_aabb().as_f32();
            if pos.contains(
                self.solver_camera
                    .screen_to_world(SCREEN_SIZE.as_f32(), self.cursor_position_game),
            ) {
                pos = pos.extend_uniform(0.3);
                self.client_state.hovering_smth = true;
            }

            geng_utils::texture::DrawTexture::new(&texture)
                .transformed(mat3::scale(vec2(if flip { -1.0 } else { 1.0 }, 1.0)))
                .fit_width(pos, 0.0)
                .draw(&self.solver_camera, &self.context.geng, framebuffer);
        }

        // Book
        if let Focus::Book = self.client_state.focus {
            let book_pos =
                Aabb2::point(vec2(1280.0, 480.0)).extend_symmetric(vec2(1063.0, 742.0) / 2.0);
            let draw = geng_utils::texture::DrawTexture::new(&sprites.book_open)
                .fit(book_pos, vec2(0.5, 0.5));
            let book_pos = draw.target;
            draw.draw(&self.camera, &self.context.geng, framebuffer);

            let book_pos = Aabb2::from_corners(vec2(100.0, -180.0), vec2(460.0, -630.0))
                .translate(book_pos.top_left());

            let font = self.context.geng.default_font();
            self.context.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::Text::unit(&**font, &assets.dispatcher.book_text, assets.palette.text)
                    .fit_into(book_pos),
            );
        }

        // Explosion
        if let Some((pos, time)) = self.client_state.explosion {
            let frames = &assets.solver.sprites.explosion;
            let frame = (time.as_f32() * frames.len() as f32).floor() as usize;
            if let Some(frame) = frames.get(frame) {
                geng_utils::texture::DrawTexture::new(&frame.texture)
                    .fit(
                        Aabb2::point(pos.as_f32()).extend_uniform(100.0),
                        vec2(0.5, 0.5),
                    )
                    .draw(&self.camera, &self.context.geng, framebuffer);
            }
        }

        // Head
        let t = (self.time.as_f32() / 30.0).fract();
        let t = t.min(1.0 - t) * 2.0;
        let t = (t - 0.45) / 0.1;
        let t = t.clamp(0.0, 0.99);
        let frame = (t * sprites.head.len() as f32).floor() as usize;
        geng_utils::texture::DrawTexture::new(sprites.head.get(frame).unwrap_or(&sprites.head[0]))
            .fit(
                Aabb2::ZERO
                    .extend_positive(SCREEN_SIZE.as_f32())
                    .translate(vec2(0.0, -100.0)),
                vec2(0.5, 0.0),
            )
            .draw(&self.camera, &self.context.geng, framebuffer);
    }

    fn cursor_press(&mut self) {
        let assets = self.context.assets.get();

        if let Some(novella) = &mut self.client_state.novella {
            novella.fast = true;
            novella.next_char_in -= 0.1;
            if novella.is_line_done {
                novella.line += 1;
                novella.character = 0;
                novella.is_line_done = false;
                novella.fast = false;
            }
            return;
        }

        if self.ui.turn_left.contains(self.cursor_position_game) {
            assets.sounds.click.play();
            self.client_state.active_side = self.client_state.active_side.cycle_left();
            return;
        } else if self.ui.turn_right.contains(self.cursor_position_game) {
            assets.sounds.click.play();
            self.client_state.active_side = self.client_state.active_side.cycle_right();
            return;
        }

        let level = assets
            .dispatcher
            .level
            .get_side(self.client_state.active_side);
        for (item_index, (item, _)) in level.items.iter().enumerate() {
            let Some(&hitbox) = self
                .ui
                .items_layout
                .get(&(self.client_state.active_side, item_index))
            else {
                continue;
            };
            if hitbox.contains(self.cursor_position_game) {
                match item {
                    DispatcherItem::DoorSign => {
                        assets.sounds.click.play();
                        self.state.door_sign_open = !self.state.door_sign_open;
                        self.connection
                            .send(ClientMessage::SyncDispatcherState(self.state.clone()));
                    }
                    DispatcherItem::Monitor => {
                        assets.sounds.click.play();
                        drop(assets);
                        self.change_focus(Focus::Monitor);
                        break;
                    }
                    DispatcherItem::ButtonStation
                        if !self.state.button_station_open
                            || !self
                                .ui
                                .button_station_inside
                                .contains(self.cursor_position_game) =>
                    {
                        assets.sounds.click.play();
                        self.state.button_station_open = !self.state.button_station_open;
                        self.connection
                            .send(ClientMessage::SyncDispatcherState(self.state.clone()));
                    }
                    DispatcherItem::Bfb => {
                        assets.sounds.button.play();
                        if self.client_state.bfb_pressed.is_none() {
                            self.client_state.bfb_pressed = Some(FTime::ZERO);
                        }
                    }
                    DispatcherItem::ButtonYellow
                    | DispatcherItem::ButtonGreen
                    | DispatcherItem::ButtonSalad
                    | DispatcherItem::ButtonPink
                    | DispatcherItem::ButtonBlue
                    | DispatcherItem::ButtonWhite
                    | DispatcherItem::ButtonPurple
                    | DispatcherItem::ButtonOrange
                    | DispatcherItem::ButtonCyan
                        if self.state.button_station_open =>
                    {
                        let sound = match item {
                            DispatcherItem::ButtonYellow => &assets.sounds.kick,
                            DispatcherItem::ButtonGreen => &assets.sounds.psh,
                            DispatcherItem::ButtonSalad => &assets.sounds.spit,
                            DispatcherItem::ButtonPink => &assets.sounds.k,
                            DispatcherItem::ButtonBlue => &assets.sounds.liproll,
                            DispatcherItem::ButtonWhite => &assets.sounds.oo,
                            DispatcherItem::ButtonPurple => &assets.sounds.duck,
                            DispatcherItem::ButtonOrange => &assets.sounds.clop,
                            DispatcherItem::ButtonCyan => &assets.sounds.button,
                            _ => &assets.sounds.button,
                        };
                        sound.play();
                        self.client_state
                            .buttons_pressed
                            .entry(*item)
                            .or_insert(FTime::ZERO);
                    }
                    DispatcherItem::RealMouse => {
                        assets.sounds.mouse.play();
                    }
                    DispatcherItem::Cactus => {
                        assets.sounds.cactus.play();
                        self.context
                            .music
                            .fade_temporarily(0.1, time::Duration::from_secs_f64(10.0));
                    }
                    DispatcherItem::Book => {
                        assets.sounds.book.play();
                        drop(assets);
                        self.change_focus(Focus::Book);
                        break;
                    }
                    _ => {}
                }
            }
        }

        let assets = self.context.assets.get();
        if let Focus::Monitor = self.client_state.focus {
            if self.state.monitor_unlocked {
                if let Some(file) = self
                    .ui
                    .files
                    .iter()
                    .position(|file| file.contains(self.cursor_position_game))
                {
                    // Open file
                    assets.sounds.click.play();
                    if file == 4 {
                        // Open novella
                        if self.client_state.novella.is_none() {
                            self.client_state.novella = Some(NovellaState::new(&assets));
                        }
                    }
                    self.client_state.opened_file = Some(file);
                    self.client_state.opened_meme = None;
                } else if let Some(meme) = &mut self.client_state.opened_meme {
                    let total_memes = assets.dispatcher.sprites.memes.len();
                    if self.ui.meme_prev.contains(self.cursor_position_game) {
                        *meme = meme.checked_sub(1).unwrap_or(total_memes - 1);
                    } else if self.ui.meme_next.contains(self.cursor_position_game) {
                        *meme = meme.add(1);
                        if *meme >= total_memes {
                            *meme = 0;
                        }
                    }
                } else if let Some(meme) = self.ui.meme_folder
                    && meme.contains(self.cursor_position_game)
                {
                    self.client_state.opened_meme = Some(0);
                    self.client_state.opened_file = None;
                }
            } else if self.ui.user_icon.contains(self.cursor_position_game) {
                assets.sounds.click.play();
                // TODO: smth
            }
        }

        if let DispatcherViewSide::Front = self.client_state.active_side
            && let Some(player) = &self.solver_player
        {
            let pos = player.collider.compute_aabb().as_f32();
            if pos.contains(
                self.solver_camera
                    .screen_to_world(SCREEN_SIZE.as_f32(), self.cursor_position_game),
            ) {
                self.solver_state.popped = true;
                self.connection
                    .send(ClientMessage::SyncSolverState(self.solver_state.clone()));
                let pos = match self
                    .solver_camera
                    .world_to_screen(SCREEN_SIZE.as_f32(), player.collider.position.as_f32())
                {
                    Ok(v) | Err(v) => v,
                };
                self.client_state.explosion = Some((pos, FTime::ZERO));
            }
        }
    }

    fn change_focus(&mut self, focus: Focus) {
        if self.client_state.focus == focus {
            return;
        }

        let (fov, center) = match focus {
            Focus::Whole | Focus::Book => (SCREEN_SIZE.y as f32, SCREEN_SIZE.as_f32() / 2.0),
            Focus::Monitor => (
                self.ui.monitor_inside.height() + 50.0,
                self.ui.monitor_inside.center() + vec2(0.0, -20.0),
            ),
        };
        self.camera_fov.target = fov;
        self.camera_center.target = center;
        self.client_state.focus = focus;
    }

    fn press_digit(&mut self, digit: usize) {
        if self.client_state.focus == Focus::Monitor
            && !self.state.monitor_unlocked
            && self.client_state.login_code.len() < 3
        {
            self.client_state.login_code.push(digit);
        }
    }

    fn press_escape(&mut self) {
        match self.client_state.focus {
            Focus::Book => {
                self.change_focus(Focus::Whole);
            }
            Focus::Monitor => {
                if self.client_state.opened_file.take().is_some() {
                    return;
                }
                if self.client_state.opened_meme.take().is_some() {
                    return;
                }
                self.change_focus(Focus::Whole);
            }
            _ => (),
        }
    }

    fn press_backspace(&mut self) {
        if self.client_state.focus == Focus::Monitor && !self.state.monitor_unlocked {
            self.client_state.login_code.pop();
        }
    }

    fn press_enter(&mut self) {
        if self.client_state.focus == Focus::Monitor && !self.state.monitor_unlocked {
            if self.client_state.login_code == vec![6, 6, 6] {
                self.unlock_monitor();
            } else {
                // TODO
            }
        }
    }

    fn unlock_monitor(&mut self) {
        self.state.monitor_unlocked = true;
        self.connection
            .send(ClientMessage::SyncDispatcherState(self.state.clone()));
    }

    fn handle_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Ping
            | ServerMessage::RoomJoined(..)
            | ServerMessage::StartGame(..)
            | ServerMessage::YourToken(_)
            | ServerMessage::SyncRoomPlayers(_) => {}
            ServerMessage::Error(error) => log::error!("Server error: {error}"),
            ServerMessage::SyncDispatcherState(dispatcher_state) => self.state = dispatcher_state,
            ServerMessage::SyncSolverState(solver_state) => self.solver_state = solver_state,
            ServerMessage::SyncSolverPlayer(player) => self.solver_player = Some(player),
            ServerMessage::GameCrash(_) => {
                // TODO
            }
        }
    }

    fn update_buttons(&mut self, delta_time: FTime) {
        if let Some(time) = &mut self.client_state.bfb_pressed {
            *time += delta_time;
            if time.as_f32() > 1.0 {
                self.connection.send(ClientMessage::CrashOther(
                    "твой друг нажал на большую красную кнопку".into(),
                ));
                self.client_state.bfb_pressed = None;
            }
        }
        for (item, time) in &mut self.client_state.buttons_pressed {
            *time += delta_time;
            if time.as_f32() > 1.0 {
                if self.solver_state.levels_completed == 3 {
                    self.client_state.bubble_buttons += 1;
                    if self.client_state.bubble_buttons == 5 {
                        self.solver_state.levels_completed += 1;
                        self.connection
                            .send(ClientMessage::SyncSolverState(self.solver_state.clone()));
                    }
                }

                match item {
                    DispatcherItem::ButtonSalad => {
                        if self.state.monitor_unlocked && self.solver_state.levels_completed == 0 {
                            self.connection.send(ClientMessage::CrashOther(
                                "твой друг нажал на салатовую кнопку".into(),
                            ));
                        }
                    }
                    DispatcherItem::ButtonYellow => {
                        if self.state.monitor_unlocked && self.solver_state.levels_completed == 0 {
                            self.solver_state.levels_completed += 1;
                            self.connection
                                .send(ClientMessage::SyncSolverState(self.solver_state.clone()));
                        }
                    }
                    DispatcherItem::ButtonGreen => {
                        if self.solver_state.trashcan_evil
                            && self.solver_state.current_level == 2
                            && self.solver_state.levels_completed == 2
                        {
                            self.solver_state.trashcan_evil = false;
                            self.connection
                                .send(ClientMessage::SyncSolverState(self.solver_state.clone()));
                        }
                    }
                    DispatcherItem::ButtonCyan => {
                        if self.solver_state.current_level == 4
                            && self.solver_state.levels_completed == 4
                        {
                            self.solver_state.levels_completed += 1;
                            self.connection
                                .send(ClientMessage::SyncSolverState(self.solver_state.clone()));
                        }
                    }
                    _ => {}
                }
            }
        }
        self.client_state
            .buttons_pressed
            .retain(|_, time| time.as_f32() < 1.0);
    }
}

impl geng::State for GameDispatcher {
    fn update(&mut self, delta_time: f64) {
        if let Some(Ok(message)) = self.connection.try_recv() {
            self.handle_message(message);
        }

        let delta_time = delta_time as f32;
        self.camera_fov.update(delta_time);
        self.camera.fov = Camera2dFov::Vertical(self.camera_fov.current);
        self.camera_center.update(delta_time);
        self.camera.center = self.camera_center.current;

        let delta_time = FTime::new(delta_time);
        self.time += delta_time;
        self.update_buttons(delta_time);

        if let Some((_, timer)) = &mut self.client_state.explosion {
            *timer += delta_time;
            if timer.as_f32() > 1.0 {
                if self.solver_state.popped {
                    panic!("тебе конец, и игре тоже");
                }

                // panic!("ты взорвался");
            }
        }

        if let Some(novella) = &mut self.client_state.novella {
            let assets = self.context.assets.get();
            let sprites = &assets.dispatcher.sprites.novella;
            let text = &assets.dispatcher.novella;
            if let Some(line) = text.lines().nth(novella.line) {
                match line {
                    "/спрайт_нейтральный" => {
                        novella.sprite = sprites.neutral.clone();
                        novella.line += 1;
                    }
                    "/спрайт_удивленный" => {
                        novella.sprite = sprites.surprised.clone();
                        novella.line += 1;
                    }
                    "/спрайт_злой" => {
                        novella.sprite = sprites.angry.clone();
                        novella.line += 1;
                    }
                    _ => {}
                }

                novella.next_char_in -= delta_time.as_f32();
                while novella.next_char_in <= 0.0 {
                    novella.character += 1;
                    novella.next_char_in += if novella.fast { 0.1 } else { 0.2 };
                    if novella.character >= line.chars().count() {
                        novella.is_line_done = true;
                    }
                }
            } else {
                self.client_state.novella = None;
            }
        }
    }

    fn handle_event(&mut self, event: geng::Event) {
        match event {
            geng::Event::CursorMove { position } => {
                self.cursor_position_raw = position;
                let pos = (position.as_f32() - self.screen.bottom_left()) / self.screen.size()
                    * SCREEN_SIZE.as_f32();
                self.cursor_position_game = self.camera.screen_to_world(SCREEN_SIZE.as_f32(), pos);
            }
            geng::Event::MousePress {
                button: geng::MouseButton::Left,
            } => {
                self.cursor_press();
            }
            geng::Event::KeyPress { key } => match key {
                geng::Key::F11 => {
                    self.context.geng.window().toggle_fullscreen();
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
            },
            _ => (),
        }
    }

    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size();
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        let was_hovering = self.client_state.hovering_smth;
        self.draw_game();
        if !was_hovering && self.client_state.hovering_smth {
            self.context.assets.get().sounds.hover.play();
        }

        let draw = geng_utils::texture::DrawTexture::new(&self.final_texture)
            .fit_screen(vec2(0.5, 0.5), framebuffer);
        self.screen = draw.target;
        draw.draw(&geng::PixelPerfectCamera, &self.context.geng, framebuffer);
    }
}

impl DispatcherItem {
    pub fn is_interactable(&self) -> bool {
        match self {
            DispatcherItem::Door => false,
            DispatcherItem::DoorSign => true,
            DispatcherItem::Table => false,
            DispatcherItem::Monitor => true,
            DispatcherItem::RealMouse => true,
            DispatcherItem::Cactus => true,
            DispatcherItem::Book => true,
            DispatcherItem::TheSock => true,
            DispatcherItem::ButtonStation => true,
            DispatcherItem::Bfb
            | DispatcherItem::ButtonYellow
            | DispatcherItem::ButtonGreen
            | DispatcherItem::ButtonSalad
            | DispatcherItem::ButtonPink
            | DispatcherItem::ButtonBlue
            | DispatcherItem::ButtonWhite
            | DispatcherItem::ButtonPurple
            | DispatcherItem::ButtonOrange
            | DispatcherItem::ButtonCyan => true,
            DispatcherItem::Tea => false,
        }
    }
}

fn draw_text(
    font: &Font,
    text: &str,
    font_size: f32,
    color: Rgba<f32>,
    position: Aabb2<f32>,
    camera: &Camera2d,
    framebuffer: &mut ugli::Framebuffer,
) {
    let lines = crate::util::wrap_text(font, text, position.width() / font_size);
    let row = position.align_aabb(vec2(position.width(), font_size), vec2(0.5, 1.0));
    let rows = row.stack(vec2(0.0, -row.height()), lines.len());

    for (line, position) in lines.into_iter().zip(rows) {
        font.draw(
            framebuffer,
            camera,
            line,
            position.align_pos(vec2(0.0, 0.5)),
            crate::render::util::TextRenderOptions {
                size: font_size,
                color,
                align: vec2(0.0, 0.5),
                ..default()
            },
        );
    }
}
