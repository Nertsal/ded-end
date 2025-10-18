use crate::model::*;

use geng::prelude::*;

pub type ClientId = i64;

// pub type ClientConnection = geng::net::client::Connection<ServerMessage, ClientMessage>;

#[derive(Clone)]
pub struct ClientConnection {
    inner: Rc<RefCell<geng::net::client::Connection<ServerMessage, ClientMessage>>>,
}

impl ClientConnection {
    pub async fn connect(addr: &str) -> anyhow::Result<Self> {
        let conn = geng::net::client::connect(addr).await?;
        Ok(Self {
            inner: Rc::new(RefCell::new(conn)),
        })
    }

    pub fn send(&self, message: ClientMessage) {
        self.inner.borrow_mut().send(message);
    }

    pub fn try_recv(&self) -> Option<anyhow::Result<ServerMessage>> {
        self.inner.borrow_mut().try_recv()
    }
}

impl Stream for ClientConnection {
    type Item = anyhow::Result<ServerMessage>;
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        Stream::poll_next(
            unsafe {
                self.map_unchecked_mut(|pin| {
                    &mut *((&mut *pin.inner.borrow_mut()) as *mut _) as &mut _
                })
            },
            cx,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Ping,
    Error(String),
    YourToken(String),
    RoomJoined(RoomInfo),
    SyncRoomPlayers(usize),
    StartGame(GameRole),
    SyncDispatcherState(DispatcherState),
    SyncSolverState(SolverState),
    SyncSolverPlayer(Player),
    GameCrash(bool, String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Pong,
    Login(String),
    CreateRoom,
    JoinRoom(String),
    SelectRole(GameRole),
    SyncDispatcherState(DispatcherState),
    SyncSolverState(SolverState),
    SyncSolverPlayer(Player),
    CrashOther(bool, String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub players: usize,
}
