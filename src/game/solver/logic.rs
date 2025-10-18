use super::*;

impl GameSolver {
    pub fn update(&mut self, delta_time: FTime) {
        if let Some(Ok(message)) = self.connection.try_recv() {
            self.handle_message(message);
        }

        self.client_state.time += delta_time;

        if let Some((timer, _)) = &mut self.client_state.dedend {
            // DED END
            if let Some(timer) = timer {
                *timer -= delta_time;
                if *timer <= FTime::ZERO {
                    self.reload_level();
                }
            }
            return;
        }

        {
            let window = self.context.geng.window();
            let controls = &self.context.assets.get().solver.controls;
            if geng_utils::key::is_key_pressed(window, &controls.move_left) {
                self.player_control.move_dir += vec2(-1.0, 0.0).as_r32();
            }
            if geng_utils::key::is_key_pressed(window, &controls.move_right) {
                self.player_control.move_dir += vec2(1.0, 0.0).as_r32();
            }
            if geng_utils::key::is_key_pressed(window, &controls.jump) {
                self.player_control.hold_jump = true;
            }
        }

        if self.state.popped && self.client_state.explosion.is_none() {
            self.client_state.explosion =
                Some((self.client_state.player.collider.position, FTime::ZERO));
        }

        if let Some(spin) = &mut self.client_state.grandson_spin {
            *spin += Angle::from_degrees(r32(360.0) * delta_time);
            if spin.as_degrees().as_f32() > 360.0 {
                self.client_state.grandson_spin = None;
            }
        }
        if let Some(time) = &mut self.client_state.grandpa_drill {
            let t = *time;
            *time += delta_time;
            if t.as_f32() <= 1.0 && time.as_f32() > 1.0 {
                self.client_state.platforms.clear();
                self.client_state.level_static_colliders.swap_remove(0);
                if self.state.levels_completed == 2 {
                    self.state.levels_completed += 1;
                    self.connection
                        .send(ClientMessage::SyncSolverState(self.state.clone()));
                }
            }
        }

        if let Some((pos, timer)) = &mut self.client_state.explosion {
            *timer += delta_time;
            if timer.as_f32() > 0.5 {
                if self.state.popped {
                    self.game_crash(false, "тебе конец, и игре тоже");
                    return;
                }

                if (self.client_state.player.collider.position - *pos)
                    .len()
                    .as_f32()
                    < 2.5
                {
                    self.game_crash(true, "ты взорвался");
                    return;
                }
            }
            if timer.as_f32() > 1.0 {
                self.client_state.explosion = None;
                if self.state.current_level == 1 && !self.state.is_exit_open() {
                    self.state.levels_completed += 1;
                    self.connection
                        .send(ClientMessage::SyncSolverState(self.state.clone()));
                }
            }
        }

        self.update_projectiles(delta_time);
        self.update_player(delta_time);
        self.update_items(delta_time);
        self.update_balls(delta_time);
    }

    pub fn update_projectiles(&mut self, delta_time: FTime) {
        let mut remove_projs = Vec::new();
        for (proj_i, proj) in self.client_state.projectiles.iter_mut().enumerate() {
            proj.collider.position += proj.velocity * delta_time;
            if self
                .client_state
                .level_static_colliders
                .iter()
                .any(|col| proj.collider.check(col))
            {
                remove_projs.push(proj_i);
            } else if proj.collider.check(&self.client_state.player.collider) {
                self.game_crash(true, "ты попался карасю");
                return;
                // remove_projs.push(proj_i);
            }
        }

        for i in remove_projs.into_iter().rev() {
            self.client_state.projectiles.swap_remove(i);
        }
    }

    pub fn update_balls(&mut self, delta_time: FTime) {
        for (ball, _) in &mut self.client_state.bubble_balls {
            let mut any_collision = false;
            for static_col in self
                .client_state
                .level_static_colliders
                .iter()
                .chain(&self.client_state.platforms)
            {
                if let Some(collision) = ball.collide(static_col) {
                    ball.position -= collision.normal * collision.penetration;
                    any_collision = true;
                }
            }
            if !any_collision {
                ball.position += vec2(0.0, -2.0).as_r32() * delta_time;
            }
        }

        let items_count = self.client_state.bubble_balls.len();
        for i in 0..items_count {
            for j in i + 1..items_count {
                if let Ok([(ball, _), (other, _)]) =
                    self.client_state.bubble_balls.get_disjoint_mut([i, j])
                    && let Some(collision) = ball.collide(other)
                {
                    let offset = collision.normal * collision.penetration * r32(0.5);
                    ball.position -= offset;
                    other.position += offset;
                }
            }
        }
    }

    pub fn update_items(&mut self, delta_time: FTime) {
        // Item movement
        for item in &mut self.client_state.items {
            if item.has_gravity {
                let collision = self
                    .client_state
                    .level_static_colliders
                    .iter()
                    .chain(&self.client_state.platforms)
                    .filter_map(|static_col| item.collider.collide(static_col))
                    .max_by_key(|col| col.penetration);
                match collision {
                    None => {
                        item.collider.position += vec2(0.0, -5.0).as_r32() * delta_time;
                    }
                    Some(collision) => {
                        item.collider.position -= collision.normal * collision.penetration;
                    }
                }
            }

            if self.state.current_level == 4
                && self.state.levels_completed <= 4
                && item.kind == SolverItemKind::Fish
            {
                // Shoot projectiles
                self.client_state.fish_cooldown -= delta_time;
                if self.client_state.fish_cooldown.as_f32() <= 0.0 {
                    self.client_state.fish_cooldown += FTime::new(0.7);
                    let position = item.collider.position;
                    self.client_state.projectiles.push(Projectile {
                        collider: Collider::circle(position, r32(0.2)),
                        velocity: (self.client_state.player.collider.position - position)
                            .normalize_or_zero()
                            * r32(5.0),
                    });
                }
            }
        }

        // Item collision
        let items_count = self.client_state.items.len();
        let mut remove_items = Vec::new();
        for i in 0..items_count {
            for j in i + 1..items_count {
                if let Ok([item, other]) = self.client_state.items.get_disjoint_mut([i, j]) {
                    let check_combination = |a, b| {
                        item.kind == a && other.kind == b || item.kind == b && other.kind == a
                    };

                    use crate::assets::SolverItemKind::*;
                    if check_combination(Fish, CinderBlock) && item.collider.check(&other.collider)
                    {
                        // Explosion
                        remove_items.extend([i, j]);
                        self.client_state.explosion = Some((item.collider.position, FTime::ZERO));
                    }
                }
            }
        }
        remove_items.sort();
        for i in remove_items.into_iter().rev() {
            self.client_state.items.swap_remove(i);
        }
    }
}
