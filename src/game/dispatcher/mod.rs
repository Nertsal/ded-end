mod render;

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
            next_char_in: 0.2,
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

    fn change_side(&mut self, side: DispatcherViewSide) {
        self.client_state.active_side = side;
        if self.client_state.focus == Focus::Book {
            self.change_focus(Focus::Whole);
        }
    }

    fn cursor_press(&mut self) {
        let assets = self.context.assets.get();

        if let Some(novella) = &mut self.client_state.novella {
            if novella.fast {
                novella.next_char_in -= 0.2;
            }
            novella.fast = true;
            if novella.is_line_done {
                novella.line += 1;
                novella.next_char_in = 0.0;
                novella.character = 0;
                novella.is_line_done = false;
                novella.fast = false;
            }
            return;
        }

        if self.ui.turn_left.contains(self.cursor_position_game) {
            assets.sounds.click.play();
            drop(assets);
            self.change_side(self.client_state.active_side.cycle_left());
            return;
        } else if self.ui.turn_right.contains(self.cursor_position_game) {
            assets.sounds.click.play();
            drop(assets);
            self.change_side(self.client_state.active_side.cycle_right());
            return;
        }

        let mut change_focus = self.client_state.focus;

        let level = assets
            .dispatcher
            .level
            .get_side(self.client_state.active_side);
        let mut monitor = Aabb2::ZERO;
        let mut book = Aabb2::ZERO;
        for (item_index, (item, _)) in level.items.iter().enumerate() {
            let Some(&hitbox) = self
                .ui
                .items_layout
                .get(&(self.client_state.active_side, item_index))
            else {
                continue;
            };

            match item {
                DispatcherItem::Monitor => monitor = hitbox,
                DispatcherItem::Book => book = hitbox,
                _ => {}
            }

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
                        change_focus = Focus::Monitor;
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
                        change_focus = Focus::Book;
                    }
                    _ => continue,
                }
                break; // Only one item per click
            }
        }

        if let Focus::Monitor = self.client_state.focus {
            if self.state.monitor_unlocked {
                if let Some(file) = self
                    .ui
                    .files
                    .iter()
                    .position(|file| file.contains(self.cursor_position_game))
                    && file <= self.solver_state.current_level
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
            } else if !monitor.contains(self.cursor_position_game) {
                // Close monitor
                change_focus = Focus::Whole;
            }
        }

        if let Focus::Book = self.client_state.focus
            && !book.contains(self.cursor_position_game)
        {
            // Close book
            change_focus = Focus::Whole;
        }

        drop(assets);
        self.change_focus(change_focus);

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
                    novella.next_char_in += if novella.fast { 0.02 } else { 0.05 };
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
