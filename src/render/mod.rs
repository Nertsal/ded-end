pub mod mask;
pub mod texture_atlas;
pub mod util;

use crate::{assets::Font, context::*, ui::layout::AreaOps};

use geng::prelude::*;

pub type Color = Rgba<f32>;

pub fn draw_text(
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
