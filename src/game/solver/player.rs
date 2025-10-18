use super::*;

pub struct PlayerControl {
    pub jump: bool,
    pub hold_jump: bool,
    pub move_dir: vec2<FCoord>,
    pub pickup: bool,
}

impl PlayerControl {
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl Default for PlayerControl {
    fn default() -> Self {
        Self {
            jump: false,
            hold_jump: false,
            move_dir: vec2::ZERO,
            pickup: false,
        }
    }
}

impl Player {
    pub fn update_timers(&mut self, delta_time: FTime) {
        self.animation_time += delta_time;

        // Coyote Time
        if let Some(time) = &mut self.coyote_time {
            *time -= delta_time;
            if *time <= FTime::ZERO {
                self.coyote_time = None;
            }
        }

        // Jump Buffer
        if let Some(time) = &mut self.jump_buffer {
            *time -= delta_time;
            if *time <= FTime::ZERO {
                self.jump_buffer = None;
            }
        }

        // Control timeout
        if let Some(time) = &mut self.control_timeout {
            // No horizontal control
            *time -= delta_time;
            if *time <= FTime::ZERO {
                self.control_timeout = None;
            }
        }
    }

    pub fn feet_collider(&self) -> Collider {
        let aabb = self.collider.compute_aabb();
        Collider::aabb(
            aabb.extend_symmetric(-vec2(aabb.width() * r32(0.05), r32(0.0)))
                .extend_up(-aabb.height() * r32(0.8)),
        )
    }

    pub fn animation_state(&self) -> PlayerAnimationState {
        match self.state {
            PlayerState::Grounded => {
                if self.velocity.x.abs() > r32(0.01) {
                    PlayerAnimationState::Running
                } else {
                    PlayerAnimationState::Idle
                }
            }
            PlayerState::Airborn => PlayerAnimationState::Jumping,
        }
    }
}

impl GameSolver {
    pub fn player_respawn(&mut self) {
        self.client_state.level_static_colliders.clear();
        let assets = self.context.assets.get();
        let Some(level) = assets.solver.levels.get(self.state.current_level) else {
            return;
        };

        let player = &mut self.client_state.player;
        player.collider.position =
            level.spawnpoint + vec2(r32(0.0), player.collider.compute_aabb().height() / r32(2.0));
        player.velocity = vec2::ZERO;
    }

    pub fn update_player(&mut self, delta_time: FTime) {
        let anim_state = self.client_state.player.animation_state();

        {
            let state = &mut self.client_state;
            let rules = &self.context.assets.get().solver.rules;
            state.player.update_timers(delta_time);

            // Update Jump Buffer
            if self.player_control.jump {
                state.player.jump_buffer = Some(rules.buffer_time);
            }

            // Update Jump Hold
            if state.player.can_hold_jump && !self.player_control.hold_jump {
                state.player.can_hold_jump = false;
            }

            // Update look direction
            let player = &mut state.player;
            if player.facing_left && player.velocity.x > FCoord::ZERO
                || !player.facing_left && player.velocity.x < FCoord::ZERO
            {
                player.facing_left = !player.facing_left;
            }

            // Apply gravity
            state.player.velocity += rules.gravity * delta_time;
        }

        if self.player_control.pickup {
            if let Some(mut item) = self.client_state.picked_up_item.take() {
                // Drop item
                let dir = if self.client_state.player.facing_left {
                    vec2(-1.0, 0.0)
                } else {
                    vec2(1.0, 0.0)
                }
                .as_r32();
                item.collider.position =
                    self.client_state.player.collider.position + dir * r32(0.5);

                let mut disappear = false;
                for other in &self.client_state.items {
                    let check = |a, b| {
                        item.kind == a && other.kind == b && item.collider.check(&other.collider)
                    };

                    use SolverItemKind::*;
                    if check(Recycle, Grandson) {
                        self.client_state.grandson_spin = Some(Angle::ZERO);
                    } else if check(Recycle, Grandpa) {
                        self.client_state.grandpa_drill = Some(FTime::ZERO);
                    } else if check(Grandpa, Trashcan) {
                        self.game_crash("ДЕДЭНД: вспомни с кем честь имеешь, скорлупа");
                        return;
                    } else if check(Grandson, Trashcan) {
                        disappear = true;
                    }
                }

                if !disappear {
                    self.client_state.items.push(item);
                }
            } else if let Some(i) = self.client_state.interact_item
                && let Some(item) = self.client_state.items.get(i)
            {
                if item.kind == SolverItemKind::BubbleCode {
                } else {
                    // Pick up an item
                    self.client_state.picked_up_item = Some(self.client_state.items.swap_remove(i));
                }
            }
        }

        self.player_variable_jump(delta_time);
        self.player_horizontal_control(delta_time);
        self.player_jump(delta_time);

        self.player_move(delta_time);
        self.player_update_state();

        if anim_state != self.client_state.player.animation_state() {
            self.client_state.player.animation_time = FTime::ZERO;
        }

        self.check_transition();
        self.check_out_of_bounds();

        self.client_state.interact_item = self.client_state.items.iter().position(|item| {
            (item.can_pickup || item.kind == SolverItemKind::BubbleCode)
                && item.collider.check(&self.client_state.player.collider)
                && !(self.state.trashcan_evil && matches!(item.kind, SolverItemKind::Recycle))
        });

        if self.state.current_level == 5 {
            self.connection.send(ClientMessage::SyncSolverPlayer(
                self.client_state.player.clone(),
            ));
        }

        self.player_control.take();
    }

    fn player_variable_jump(&mut self, delta_time: FTime) {
        let state = &mut self.client_state;
        let rules = &self.context.assets.get().solver.rules;

        // Variable jump height
        if state.player.velocity.y < FCoord::ZERO {
            // Faster drop
            state.player.velocity.y +=
                rules.gravity.y * (rules.fall_multiplier - FCoord::ONE) * delta_time;
            let cap = rules.free_fall_speed;
            state.player.velocity.y = state.player.velocity.y.clamp_abs(cap);
        } else if state.player.velocity.y > FCoord::ZERO
            && !(self.player_control.hold_jump && state.player.can_hold_jump)
        {
            // Low jump
            state.player.velocity.y +=
                rules.gravity.y * (rules.low_multiplier - FCoord::ONE) * delta_time;
        }
    }

    fn player_horizontal_control(&mut self, delta_time: FTime) {
        let state = &mut self.client_state;
        let rules = &self.context.assets.get().solver.rules;

        if state.player.control_timeout.is_some() {
            return;
        }

        // Horizontal speed control
        let current = state.player.velocity.x;
        let max_speed = rules.move_speed;
        let target = self.player_control.move_dir.x * max_speed;

        let mut acc = FCoord::ZERO;

        // Acceleration
        let is_grounded = matches!(state.player.state, PlayerState::Grounded);
        if target == FCoord::ZERO
            || target.signum() != current.signum()
            || target.abs() > current.abs()
        {
            // Accelerate towards target
            acc += if is_grounded {
                rules.acceleration_ground
            } else {
                rules.acceleration_air
            };
        } else {
            // Target is aligned with current velocity and is higher
            // Decelerate
            acc += if is_grounded {
                rules.deceleration_ground
            } else {
                rules.deceleration_air
            };
        }

        state.player.velocity.x += (target - current).clamp_abs(acc * delta_time);
    }

    fn player_jump(&mut self, _delta_time: FTime) {
        let state = &mut self.client_state;
        let rules = &self.context.assets.get().solver.rules;

        if state.player.jump_buffer.is_none() {
            return;
        }

        // Try jump
        let jump = match state.player.state {
            PlayerState::Grounded => true,
            PlayerState::Airborn => state.player.coyote_time.is_some(),
        };
        if !jump {
            return;
        }

        // Use jump
        state.player.coyote_time = None;
        state.player.jump_buffer = None;
        state.player.can_hold_jump = true;
        let player = &mut state.player;
        let push = if self.player_control.move_dir.x == FCoord::ZERO {
            FCoord::ZERO
        } else {
            rules.jump_push * self.player_control.move_dir.x.signum()
        };
        let jump_vel = vec2(player.velocity.x + push, rules.jump_strength);
        player.velocity = jump_vel;
        player.state = PlayerState::Airborn;
        // self.world.assets.sounds.jump.play();
        // self.spawn_particles(ParticleSpawn {
        //     lifetime: Time::ONE,
        //     position: actor.collider.feet(),
        //     velocity: vec2(Coord::ZERO, Coord::ONE),
        //     amount: 3,
        //     color: Rgba::WHITE,
        //     radius: Coord::new(0.1),
        //     ..Default::default()
        // });
    }

    fn player_check_ground(&mut self) {
        let rules = &self.context.assets.get().solver.rules;
        let player = &mut self.client_state.player;
        let was_grounded = matches!(player.state, PlayerState::Grounded);
        if was_grounded {
            player.state = PlayerState::Airborn;
        }
        let update_state = (matches!(player.state, PlayerState::Airborn) || was_grounded)
            && player.velocity.y <= FCoord::ZERO;

        if update_state {
            let collider = player.feet_collider();

            if self
                .check_ground_collision(&self.client_state.player.collider, &collider)
                .is_some()
            {
                let player = &mut self.client_state.player;
                player.state = PlayerState::Grounded;
                player.coyote_time = Some(rules.coyote_time);

                // if !was_grounded {
                //     // Just landed
                //     let spawn = ParticleSpawn {
                //         lifetime: Time::ONE,
                //         position: actor.collider.feet(),
                //         velocity: vec2(Coord::ZERO, Coord::ONE) * Coord::new(0.5),
                //         amount: 3,
                //         color: Rgba::WHITE,
                //         radius: Coord::new(0.1),
                //         ..Default::default()
                //     };
                //     self.spawn_particles(spawn);
                // }
            }
        }
    }

    fn player_move(&mut self, delta_time: FTime) {
        let player = &mut self.client_state.player;
        player.collider.position += player.velocity * delta_time;

        let fix_collision = |player: &mut Player, collision: &Collision| {
            player.collider.position -= collision.normal * collision.penetration;
            player.velocity -= collision.normal * vec2::dot(player.velocity, collision.normal);
        };
        let collide_with = |player: &mut Player, other: &Collider| {
            if let Some(collision) = player.collider.collide(other) {
                fix_collision(player, &collision);
            }
        };

        // Bubbles
        let player = &mut self.client_state.player;
        let mut remove_balls = Vec::new();
        for (ball_i, (ball, _)) in self.client_state.bubble_balls.iter().enumerate() {
            if let Some(collision) = player.collider.collide(ball) {
                let velocity_offset =
                    collision.normal * vec2::dot(player.velocity, collision.normal);
                if velocity_offset.len() > r32(7.0) {
                    remove_balls.push(ball_i);
                }
                player.collider.position -= collision.normal * collision.penetration.min(r32(0.5));
                player.velocity -= velocity_offset;
            }
        }
        if !remove_balls.is_empty() {
            self.context
                .assets
                .get()
                .sounds
                .pop
                .choose(&mut thread_rng())
                .unwrap()
                .play();
        }
        for i in remove_balls.into_iter().rev() {
            self.client_state.bubble_balls.swap_remove(i);
        }

        // Static colliders
        for static_col in &self.client_state.level_static_colliders {
            collide_with(player, static_col);
        }

        // Doors
        collide_with(player, &self.client_state.door_entrance);
        if !self.state.is_exit_open() {
            collide_with(player, &self.client_state.door_exit);
        }

        // Platforms
        if player.velocity.y.as_f32() <= 0.0 {
            let collider = player.feet_collider();
            for platform in &self.client_state.platforms {
                if let Some(collision) = collider.collide(platform) {
                    fix_collision(player, &collision);
                }
            }
        }

        // Items
        for item in &mut self.client_state.items {
            if item.pushable
                && let Some(collision) = player.collider.collide(&item.collider)
            {
                let offset = collision.normal * collision.penetration;
                // let velocity_offset =
                //     collision.normal * vec2::dot(player.velocity, collision.normal);
                player.collider.position -= offset * r32(0.5);
                // player.velocity -= velocity_offset * r32(0.5);
                item.collider.position += vec2::UNIT_X * vec2::dot(vec2::UNIT_X, offset) * r32(0.5);
            }
        }
    }

    fn player_update_state(&mut self) {
        self.player_check_ground();
    }

    fn check_ground_collision(
        &self,
        collider: &Collider,
        feet_collider: &Collider,
    ) -> Option<Collision> {
        self.client_state
            .level_static_colliders
            .iter()
            .chain(&self.client_state.platforms)
            .filter_map(|static_col| feet_collider.collide(static_col))
            .chain(
                self.client_state
                    .bubble_balls
                    .iter()
                    .map(|(ball, _)| ball)
                    .filter_map(|col| collider.collide(col)),
            )
            .max_by_key(|col| col.penetration)
    }

    fn check_transition(&mut self) {
        let assets = self.context.assets.get();
        let Some(level) = assets.solver.levels.get(self.state.current_level) else {
            return;
        };
        let player = &self.client_state.player;
        if self.state.is_exit_open() && player.collider.check(&Collider::aabb(level.transition)) {
            self.state.current_level += 1;
            self.connection
                .send(ClientMessage::SyncSolverState(self.state.clone()));
            drop(assets);
            self.reload_level();
        }
    }

    fn check_out_of_bounds(&mut self) {
        let player = &self.client_state.player;
        if player.collider.position.y < r32(-50.0) {
            self.player_respawn();
        }
    }
}
