//! GPU rendering for Graphite: `wgpu` for drawing, `cosmic-text` for text.

mod atlas;
mod renderer;

pub use renderer::{rgb, Color, RenderError, Renderer};
