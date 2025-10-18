mod dispatcher;
mod font;
mod solver;

pub use self::{dispatcher::*, font::Font, solver::*};

use crate::render::Color;

use std::path::PathBuf;

use geng::prelude::*;
use geng_utils::gif::GifFrame;

#[derive(geng::asset::Load)]
pub struct Assets {
    pub sounds: SoundAssets,
    pub atlas: SpritesAtlas,
    pub shaders: ShaderAssets,
    pub palette: Palette,
    pub sprites: SpriteAssets,
    pub dispatcher: DispatcherAssets,
    pub solver: SolverAssets,
    #[load(path = "default.ttf")]
    pub font: Rc<Font>,
}

#[derive(geng::asset::Load, Serialize, Deserialize)]
#[load(serde = "ron")]
pub struct Palette {
    pub background: Color,
    pub text: Color,
}

#[derive(geng::asset::Load)]
pub struct SpriteAssets {
    pub dedend: Rc<PixelTexture>,
}

#[derive(geng::asset::Load)]
pub struct SoundAssets {
    #[load(ext = "mp3", options(looped = "true"))]
    pub music: Rc<geng::Sound>,
    #[load(ext = "mp3", options(looped = "true"))]
    pub dispatcher: Rc<geng::Sound>,
    #[load(ext = "mp3", options(looped = "true"))]
    pub boss: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub click: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub hover: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub mouse: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub book: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub button: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub cactus: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub uwu: Rc<geng::Sound>,
    #[load(ext = "mp3", list = "0..=2")]
    pub pop: Vec<Rc<geng::Sound>>,

    #[load(ext = "mp3")]
    pub clop: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub duck: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub k: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub kick: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub liproll: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub oo: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub psh: Rc<geng::Sound>,
    #[load(ext = "mp3")]
    pub spit: Rc<geng::Sound>,
}

#[derive(geng::asset::Load)]
pub struct ShaderAssets {
    pub masked: Rc<ugli::Program>,
    pub texture_ui: Rc<ugli::Program>,
}

friendly_derive::texture_atlas!(pub SpritesAtlas {
    white,

    menu,
    button_background,
    create_room,
    join_room,
    code_background,
    lobby,
    think0,
    think1,
    run0,
    run1,
});

fn load_gif(
    manager: &geng::asset::Manager,
    path: &std::path::Path,
) -> geng::asset::Future<Vec<GifFrame>> {
    let manager = manager.clone();
    let path = path.to_owned();
    async move {
        geng_utils::gif::load_gif(
            &manager,
            &path,
            geng_utils::gif::GifOptions {
                frame: geng::asset::TextureOptions {
                    filter: ugli::Filter::Nearest,
                    ..Default::default()
                },
            },
        )
        .await
    }
    .boxed_local()
}

#[derive(Clone)]
pub struct PixelTexture {
    pub path: PathBuf,
    pub texture: Rc<ugli::Texture>,
}

impl Deref for PixelTexture {
    type Target = ugli::Texture;

    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Debug for PixelTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PixelTexture")
            .field("path", &self.path)
            .field("texture", &"<texture data>")
            .finish()
    }
}

impl geng::asset::Load for PixelTexture {
    type Options = <ugli::Texture as geng::asset::Load>::Options;

    fn load(
        manager: &geng::asset::Manager,
        path: &std::path::Path,
        options: &Self::Options,
    ) -> geng::asset::Future<Self> {
        let path = path.to_owned();
        let texture = ugli::Texture::load(manager, &path, options);
        async move {
            let texture = texture.await?;
            // texture.set_filter(ugli::Filter::Nearest);
            Ok(Self {
                path,
                texture: Rc::new(texture),
            })
        }
        .boxed_local()
    }

    const DEFAULT_EXT: Option<&'static str> = Some("png");
}
