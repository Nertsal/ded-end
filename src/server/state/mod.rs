use super::*;

use crate::model::{DispatcherState, GameRole, SolverState};

use geng::prelude::{
    rand::{distributions::DistString, prelude::Distribution},
    *,
};

pub struct Client {
    pub sender: Box<dyn geng::net::Sender<ServerMessage>>,
    pub token: String,
    pub room: Option<Arc<str>>,
}

pub struct ServerState {
    test: bool,
    timer: Timer,
    next_id: ClientId,
    clients: HashMap<ClientId, Client>,
    rooms: HashMap<Arc<str>, Room>,
}

pub struct Room {
    pub code: Arc<str>,
    pub players: Vec<(ClientId, String)>,
    pub state: RoomState,
}

pub enum RoomState {
    RoleSelection { roles: HashMap<ClientId, GameRole> },
    Game(RoomGameState),
}

pub struct RoomGameState {
    pub dispatcher: DispatcherState,
    pub solver: SolverState,
}

impl RoomGameState {
    pub fn new() -> Self {
        Self {
            dispatcher: DispatcherState::new(),
            solver: SolverState::new(),
        }
    }
}

impl Room {
    pub fn new(code: Arc<str>) -> Self {
        Self {
            code,
            players: vec![],
            state: RoomState::RoleSelection {
                roles: HashMap::new(),
            },
        }
    }

    pub fn info(&self) -> RoomInfo {
        RoomInfo {
            code: self.code.to_string(),
            players: self.players.len(),
        }
    }
}

impl ServerState {
    pub const TICKS_PER_SECOND: f32 = 2.0;

    pub fn new(test: bool) -> Self {
        Self {
            test,
            timer: Timer::new(),
            next_id: 1,
            clients: HashMap::new(),
            rooms: HashMap::new(),
        }
    }

    pub fn client_connect(
        &mut self,
        mut sender: Box<dyn geng::net::Sender<ServerMessage>>,
    ) -> ClientId {
        if self.clients.is_empty() {
            self.timer.reset();
        }

        let my_id = self.next_id;
        self.next_id += 1;

        sender.send(ServerMessage::Ping);
        let token = rand::distributions::Alphanumeric.sample_string(&mut thread_rng(), 16);
        sender.send(ServerMessage::YourToken(token.clone()));

        let client = Client {
            sender,
            token,
            room: None,
        };

        self.clients.insert(my_id, client);
        my_id
    }

    pub fn client_disconnect(&mut self, client_id: ClientId) {
        let Some(client) = self.clients.remove(&client_id) else {
            return;
        };
        if let Some(code) = &client.room
            && let Some(room) = self.rooms.get_mut(code)
            && let Some(i) = room
                .players
                .iter()
                .position(|(_, token)| *token == client.token)
            && let RoomState::RoleSelection { .. } = room.state
        {
            room.players.remove(i);
        }
    }

    pub fn handle_message(&mut self, client_id: ClientId, message: ClientMessage) {
        let client = self
            .clients
            .get_mut(&client_id)
            .expect("Sender not found for client");
        match message {
            ClientMessage::Pong => {
                // client.sender.send(ServerMessage::Time(
                //     state.timer.elapsed().as_secs_f64() as f32
                // ));
                client.sender.send(ServerMessage::Ping);
            }
            ClientMessage::Login(token) => {
                client.token = token.clone();
                client.sender.send(ServerMessage::YourToken(token.clone()));
            }
            ClientMessage::CreateRoom => {
                match &client.room {
                    Some(_) => {
                        // The client already has a room, there's some desync
                        // TODO: fix desync
                    }
                    None => 'room: {
                        for _ in 0..10 {
                            let code: String =
                                rand::distributions::Uniform::new_inclusive('A', 'Z')
                                    .sample_iter(&mut thread_rng())
                                    .take(4)
                                    .collect();
                            let code: Arc<str> = code.into();
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                self.rooms.entry(code.clone())
                            {
                                client.room = Some(code.clone());
                                let mut room = Room::new(code);
                                room.players.push((client_id, client.token.clone()));
                                client.sender.send(ServerMessage::RoomJoined(room.info()));
                                e.insert(room);
                                break 'room;
                            }
                        }
                        // Failed to create a room
                        // TODO: idk
                    }
                }
            }
            ClientMessage::JoinRoom(code) => {
                let code: Arc<str> = code.to_uppercase().into();
                if let Some(room) = self.rooms.get_mut(&code) {
                    match &room.state {
                        RoomState::RoleSelection { .. } => {
                            if room.players.len() < 2 {
                                room.players.push((client_id, client.token.clone()));
                                client.room = Some(code.clone());
                                client.sender.send(ServerMessage::RoomJoined(room.info()));
                                for &(id, _) in &room.players {
                                    if id != client_id
                                        && let Some(client) = self.clients.get_mut(&id)
                                    {
                                        client.sender.send(ServerMessage::SyncRoomPlayers(
                                            room.players.len(),
                                        ));
                                    }
                                }
                            } else {
                                client
                                    .sender
                                    .send(ServerMessage::Error("room already full".into()));
                            }
                        }
                        RoomState::Game(state) => {
                            if let Some((id, _)) = room
                                .players
                                .iter_mut()
                                .find(|(_, token)| *token == client.token)
                            {
                                // Join back
                                *id = client_id;
                                client.room = Some(code.clone());
                                client.sender.send(ServerMessage::RoomJoined(room.info()));
                                client
                                    .sender
                                    .send(ServerMessage::SyncSolverState(state.solver.clone()));
                                client.sender.send(ServerMessage::SyncDispatcherState(
                                    state.dispatcher.clone(),
                                ));
                            } else {
                                client.sender.send(ServerMessage::Error(
                                    "cannot join an ongoing game".into(),
                                ));
                            }
                        }
                    }
                } else {
                    client
                        .sender
                        .send(ServerMessage::Error("non-existent room code".into()));
                }
            }
            ClientMessage::SelectRole(role) => {
                let Some(room) = client
                    .room
                    .as_ref()
                    .and_then(|code| self.rooms.get_mut(code))
                else {
                    return;
                };

                let RoomState::RoleSelection { roles } = &mut room.state else {
                    return;
                };
                log::debug!("Player {client_id} selected role {role:?}");
                roles.insert(client_id, role);

                if roles.len() == room.players.len() {
                    if roles.len() == 2 {
                        let roles_list: Vec<GameRole> = roles.values().copied().collect();
                        if roles_list[0] == roles_list[1] {
                            // Select roles randomly
                            let mut role = (GameRole::Dispatcher, GameRole::Solver);
                            if thread_rng().gen_bool(0.5) {
                                std::mem::swap(&mut role.0, &mut role.1);
                            }
                            for (role, player) in
                                [role.0, role.1].into_iter().zip(roles.values_mut())
                            {
                                *player = role;
                            }
                        }

                        let roles = roles.clone();
                        room.state = RoomState::Game(RoomGameState::new());
                        for (player, _) in &room.players {
                            if let Some(&role) = roles.get(player)
                                && let Some(client) = self.clients.get_mut(player)
                            {
                                client.sender.send(ServerMessage::StartGame(role));
                            }
                        }
                    } else if self.test && roles.len() == 1 {
                        let role = *roles.values().next().unwrap();
                        if let Some(client) = self.clients.get_mut(&room.players[0].0) {
                            room.state = RoomState::Game(RoomGameState::new());
                            client.sender.send(ServerMessage::StartGame(role));
                        }
                    }
                }
            }
            ClientMessage::SyncDispatcherState(dispatcher_state) => {
                if let Some(room) = client
                    .room
                    .as_ref()
                    .and_then(|room| self.rooms.get_mut(room))
                    && let RoomState::Game(state) = &mut room.state
                {
                    state.dispatcher = dispatcher_state.clone();
                    for &(id, _) in &room.players {
                        if client_id != id
                            && let Some(client) = self.clients.get_mut(&id)
                        {
                            client
                                .sender
                                .send(ServerMessage::SyncDispatcherState(dispatcher_state.clone()));
                        }
                    }
                }
            }
            ClientMessage::SyncSolverState(solver_state) => {
                if let Some(room) = client
                    .room
                    .as_ref()
                    .and_then(|room| self.rooms.get_mut(room))
                    && let RoomState::Game(state) = &mut room.state
                {
                    state.solver = solver_state.clone();
                    for &(id, _) in &room.players {
                        if client_id != id
                            && let Some(client) = self.clients.get_mut(&id)
                        {
                            client
                                .sender
                                .send(ServerMessage::SyncSolverState(solver_state.clone()));
                        }
                    }
                }
            }
            ClientMessage::SyncSolverPlayer(player) => {
                if let Some(room) = client
                    .room
                    .as_ref()
                    .and_then(|room| self.rooms.get_mut(room))
                    && let RoomState::Game(_) = &mut room.state
                {
                    for &(id, _) in &room.players {
                        if client_id != id
                            && let Some(client) = self.clients.get_mut(&id)
                        {
                            client
                                .sender
                                .send(ServerMessage::SyncSolverPlayer(player.clone()));
                        }
                    }
                }
            }
            ClientMessage::CrashOther(auto_reboot, message) => {
                if let Some(room) = client
                    .room
                    .as_ref()
                    .and_then(|room| self.rooms.get_mut(room))
                    && let RoomState::Game(_) = &mut room.state
                {
                    for &(id, _) in &room.players {
                        if client_id != id
                            && let Some(client) = self.clients.get_mut(&id)
                        {
                            client
                                .sender
                                .send(ServerMessage::GameCrash(auto_reboot, message.clone()));
                        }
                    }
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.rooms.retain(|_code, room| !room.players.is_empty())
    }
}
