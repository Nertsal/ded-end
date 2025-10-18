use super::*;

impl GameDispatcher {
    pub fn draw_game(&mut self) {
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
