//! GPU rendering for Graphite: `wgpu` for drawing, `cosmic-text` for text.

mod atlas;
mod cache;
pub mod icons;
mod renderer;

pub use renderer::{rgb, Color, IconFont, Image, RenderError, Renderer};
