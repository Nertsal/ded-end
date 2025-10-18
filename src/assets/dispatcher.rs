use super::*;

#[derive(geng::asset::Load)]
pub struct DispatcherAssets {
    pub sprites: DispatcherSprites,
    pub level: DispatcherLevel,
    #[load(list = "0..=4")]
    pub files: Vec<String>,
    pub book_text_left: String,
    pub book_text_right: String,
    pub novella: String,
}

#[derive(geng::asset::Load)]
pub struct DispatcherSprites {
    #[load(list = "1..=7")]
    pub head: Vec<Rc<PixelTexture>>,

    pub door: Rc<PixelTexture>,
    pub sign_open: Rc<PixelTexture>,
    pub sign_closed: Rc<PixelTexture>,

    pub table: Rc<PixelTexture>,
    pub monitor: Rc<PixelTexture>,
    pub cactus: Rc<PixelTexture>,
    pub real_mouse: Rc<PixelTexture>,
    pub book: Rc<PixelTexture>,
    pub book_open: Rc<PixelTexture>,
    pub the_sock: Rc<PixelTexture>,

    pub button_station_open: Rc<PixelTexture>,
    pub button_station_closed: Rc<PixelTexture>,
    pub button_base: Rc<PixelTexture>,
    pub button: Rc<PixelTexture>,
    pub button_pressed: Rc<PixelTexture>,
    pub button_big: Rc<PixelTexture>,
    pub button_big_pressed: Rc<PixelTexture>,

    pub tea: Rc<PixelTexture>,

    pub arrow_left: Rc<PixelTexture>,
    pub arrow_right: Rc<PixelTexture>,

    pub user_icon: Rc<PixelTexture>,
    pub login_screen: Rc<PixelTexture>,
    pub workspace: Rc<PixelTexture>,
    pub workspace_v2: Rc<PixelTexture>,
    pub file: Rc<PixelTexture>,
    pub file_window: Rc<PixelTexture>,

    pub novella: NovellaSprites,
    #[load(list = "0..=9")]
    pub memes: Vec<Rc<PixelTexture>>,
    pub meme_folder: Rc<PixelTexture>,
}

#[derive(geng::asset::Load)]
pub struct NovellaSprites {
    pub background: Rc<PixelTexture>,
    pub textbox: Rc<PixelTexture>,
    pub neutral: Rc<PixelTexture>,
    pub surprised: Rc<PixelTexture>,
    pub angry: Rc<PixelTexture>,
}

#[derive(geng::asset::Load, Clone)]
pub struct DispatcherLevel {
    pub front: DispatcherView,
    pub left: DispatcherView,
    pub right: DispatcherView,
    pub back: DispatcherView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatcherViewSide {
    Front,
    Left,
    Right,
    Back,
}

#[derive(geng::asset::Load, Serialize, Deserialize, Clone)]
#[load(serde = "ron")]
pub struct DispatcherView {
    pub items: Vec<(DispatcherItem, DispatcherItemPosition)>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum DispatcherItem {
    Door,
    DoorSign,

    Table,
    Monitor,
    RealMouse,
    Cactus,
    Book,
    TheSock,

    ButtonStation,
    Bfb,
    ButtonYellow,
    ButtonGreen,
    ButtonSalad,
    ButtonPink,
    ButtonBlue,
    ButtonWhite,
    ButtonPurple,
    ButtonOrange,
    ButtonCyan,

    Tea,
}

/// Positioning in screen-space with fixed 1920x1080 resolution.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DispatcherItemPosition {
    pub anchor: vec2<f32>,
    #[serde(default = "default_alignment")]
    pub alignment: vec2<f32>,
    pub size: Option<vec2<f32>>,
}

fn default_alignment() -> vec2<f32> {
    vec2(0.5, 0.5)
}

impl DispatcherLevel {
    pub fn get_side(&self, side: DispatcherViewSide) -> &DispatcherView {
        match side {
            DispatcherViewSide::Front => &self.front,
            DispatcherViewSide::Left => &self.left,
            DispatcherViewSide::Right => &self.right,
            DispatcherViewSide::Back => &self.back,
        }
    }
}

impl DispatcherViewSide {
    pub fn cycle_left(self) -> Self {
        match self {
            Self::Front => Self::Left,
            Self::Left => Self::Back,
            Self::Back => Self::Right,
            Self::Right => Self::Front,
        }
    }

    pub fn cycle_right(self) -> Self {
        match self {
            Self::Front => Self::Right,
            Self::Left => Self::Front,
            Self::Back => Self::Left,
            Self::Right => Self::Back,
        }
    }
}
