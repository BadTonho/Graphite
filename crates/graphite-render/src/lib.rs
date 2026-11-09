//! GPU rendering for Graphite: `wgpu` for drawing, `cosmic-text` for text.

mod atlas;
pub mod icons;
mod renderer;

pub use renderer::{rgb, Color, RenderError, Renderer};
