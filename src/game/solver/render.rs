use super::*;

impl GameSolver {
    pub fn draw_game(&mut self) {
        let framebuffer = &mut geng_utils::texture::attach_texture(
            &mut self.final_texture,
            self.context.geng.ugli(),
        );
        let assets = self.context.assets.get();
        ugli::clear(framebuffer, Some(assets.palette.background), None, None);
        let Some(level) = assets.solver.levels.get(self.state.current_level) else {
            return;
        };

        // Background
        if self.state.current_level == 0 {
            geng_utils::texture::DrawTexture::new(&assets.solver.sprites.level1)
                .fit(Aabb2::ZERO.extend_positive(LEVEL_SIZE), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        } else if self.state.current_level == 2 {
            let texture = &assets.solver.sprites.green_hint;
            geng_utils::texture::DrawTexture::new(texture)
                .fit(
                    Aabb2::point(LEVEL_SIZE * vec2(0.7, 0.5))
                        .extend_symmetric(vec2(3.0, 3.0) / 2.0),
                    vec2(0.5, 0.5),
                )
                .draw(&self.camera, &self.context.geng, framebuffer);
        } else if self.state.current_level == 3 {
            let texture = &assets.solver.sprites.bubble_tea;
            geng_utils::texture::DrawTexture::new(texture)
                .fit(Aabb2::ZERO.extend_positive(LEVEL_SIZE), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        } else if self.state.current_level == 5 {
            let texture = &assets.solver.sprites.dispatcher;
            geng_utils::texture::DrawTexture::new(texture)
                .fit(Aabb2::ZERO.extend_positive(LEVEL_SIZE), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Bounds
        if self.state.current_level != 5 {
            geng_utils::texture::DrawTexture::new(&assets.solver.sprites.level_bounds)
                .fit(Aabb2::ZERO.extend_positive(LEVEL_SIZE), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Doors
        if level.door_entrance {
            geng_utils::texture::DrawTexture::new(&assets.solver.sprites.door_closed)
                .transformed(mat3::scale(vec2(-1.0, 1.0)))
                .fit_height(self.client_state.door_entrance.compute_aabb().as_f32(), 0.0)
                .draw(&self.camera, &self.context.geng, framebuffer);
        }
        if level.door_exit {
            geng_utils::texture::DrawTexture::new(if self.state.is_exit_open() {
                &assets.solver.sprites.door_open
            } else {
                &assets.solver.sprites.door_closed
            })
            .fit_height(self.client_state.door_exit.compute_aabb().as_f32(), 1.0)
            .draw(&self.camera, &self.context.geng, framebuffer);
        }

        if self.state.current_level == 3 {
            // Bubble door
            if !self.state.solved_bubble_code
                && let Some(door) = self.client_state.level_static_colliders.last()
            {
                geng_utils::texture::DrawTexture::new(&assets.solver.sprites.bubble_door)
                    .fit_height(door.compute_aabb().as_f32(), 0.5)
                    .draw(&self.camera, &self.context.geng, framebuffer);
            }
        }

        // Platforms
        for platform in &self.client_state.platforms {
            geng_utils::texture::DrawTexture::new(&assets.solver.sprites.platform)
                .fit_width(platform.compute_aabb().as_f32(), 1.0)
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Items
        for item in &self.client_state.items {
            let texture = if let SolverItemKind::Recycle = item.kind
                && self.state.trashcan_evil
            {
                continue;
            } else if let SolverItemKind::Trashcan = item.kind
                && self.state.trashcan_evil
            {
                &assets.solver.sprites.trashcan_evil
            } else if let SolverItemKind::Fish = item.kind {
                let frames = &assets.solver.sprites.fish;
                let frame = ((self.client_state.time.as_f32() / 1.5).fract() * frames.len() as f32)
                    .floor() as usize;
                frames.get(frame).unwrap_or(&frames[0])
            } else if let SolverItemKind::CinderBlock = item.kind {
                let frames = &assets.solver.sprites.excalibur;
                let frame = ((self.client_state.time.as_f32() / 0.7).fract() * frames.len() as f32)
                    .floor() as usize;
                frames.get(frame).unwrap_or(&frames[0])
            } else {
                assets.solver.sprites.item_texture(item.kind)
            };

            let mut transform = mat3::identity();
            if let SolverItemKind::Fish = item.kind {
                transform *= mat3::scale_uniform(5.0);
            }
            if let SolverItemKind::Grandson = item.kind
                && let Some(spin) = self.client_state.grandson_spin
            {
                transform *= mat3::rotate(spin.as_f32());
            }
            if let SolverItemKind::Grandpa = item.kind
                && let Some(time) = self.client_state.grandpa_drill
            {
                let spin =
                    Angle::from_degrees(180.0 * crate::util::smoothstep(time.as_f32().min(1.0)));
                let offset = crate::util::smoothstep((time.as_f32() - 1.0).max(0.0));
                transform *= mat3::translate(vec2(0.0, -offset * 5.0)) * mat3::rotate(spin);
            }

            let draw = geng_utils::texture::DrawTexture::new(texture)
                .fit(item.collider.compute_aabb().as_f32(), vec2(0.5, 0.5))
                .transformed(transform);
            let target = draw.target;
            draw.draw(&self.camera, &self.context.geng, framebuffer);

            if let SolverItemKind::BubbleCode = item.kind {
                let code = target;
                let code = code.extend_symmetric(-code.size() * vec2(0.1, 0.15));
                let font = self.context.geng.default_font();
                for (pos, &digit) in code
                    .split_columns(4)
                    .into_iter()
                    .zip(&self.client_state.bubble_code)
                {
                    self.context.geng.draw2d().draw2d(
                        framebuffer,
                        &self.camera,
                        &draw2d::Text::unit(&**font, digit.to_string(), assets.palette.text)
                            .fit_into(pos),
                    )
                }
            }
        }

        // Balls
        for (ball, texture_i) in &self.client_state.bubble_balls {
            let texture = assets
                .solver
                .sprites
                .balls
                .get(*texture_i)
                .unwrap_or(&assets.solver.sprites.balls[0]);
            geng_utils::texture::DrawTexture::new(texture)
                .fit(ball.compute_aabb().as_f32(), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Projectile
        for projectile in &self.client_state.projectiles {
            let texture = &assets.solver.sprites.projectile;
            geng_utils::texture::DrawTexture::new(texture)
                .fit(projectile.collider.compute_aabb().as_f32(), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Player
        if !self.state.popped {
            let player = &self.client_state.player;
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
                    if player.velocity.y > FCoord::ZERO {
                        sprites[0].clone()
                    } else {
                        sprites[1].clone()
                    }
                }
            };
            let flip = !player.facing_left;
            geng_utils::texture::DrawTexture::new(&texture)
                .transformed(mat3::scale(vec2(if flip { -1.0 } else { 1.0 }, 1.0)))
                .fit_width(player.collider.compute_aabb().as_f32(), 0.0)
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Held item
        if let Some(item) = &self.client_state.picked_up_item {
            let texture = assets.solver.sprites.item_texture(item.kind);
            let mut collider = item.collider.clone();
            let dir = if self.client_state.player.facing_left {
                vec2(-1.0, 0.0)
            } else {
                vec2(1.0, 0.0)
            }
            .as_r32();
            collider.position = self.client_state.player.collider.position + dir * r32(0.5);
            geng_utils::texture::DrawTexture::new(texture)
                .fit(collider.compute_aabb().as_f32(), vec2(0.5, 0.5))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Interact hint
        if let Some(i) = self.client_state.interact_item
            && let Some(item) = self.client_state.items.get(i)
        {
            let pos = item.collider.compute_aabb().top_right();
            let pos = Aabb2::point(pos.as_f32()).extend_positive(vec2(1.0, 1.0));
            geng_utils::texture::DrawTexture::new(&assets.solver.sprites.interact)
                .fit(pos, vec2(0.0, 0.0))
                .draw(&self.camera, &self.context.geng, framebuffer);
        }

        // Explosion
        if let Some((pos, time)) = self.client_state.explosion {
            let frames = &assets.solver.sprites.explosion;
            let frame = (time.as_f32() * frames.len() as f32).floor() as usize;
            if let Some(frame) = frames.get(frame) {
                geng_utils::texture::DrawTexture::new(&frame.texture)
                    .fit(
                        Aabb2::point(pos.as_f32()).extend_uniform(2.0),
                        vec2(0.5, 0.5),
                    )
                    .draw(&self.camera, &self.context.geng, framebuffer);
            }
        }
    }
}
