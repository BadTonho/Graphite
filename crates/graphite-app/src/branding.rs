//! Embedded artwork: startup never depends on a PNG next to the executable.
pub(crate) fn window_attributes() -> winit::window::WindowAttributes {
    let pixels = include_bytes!(concat!(env!("OUT_DIR"), "/graphite-icon.rgba"));
    let icon = winit::window::Icon::from_rgba(pixels.to_vec(), 256, 256)
        .expect("build-generated icon must contain 256x256 RGBA pixels");
    let attributes = winit::window::Window::default_attributes()
        .with_title("Graphite")
        .with_window_icon(Some(icon));
    #[cfg(target_os = "linux")]
    let attributes = {
        // Both Linux backends share this name field; it sets Wayland app_id / X11 WM_CLASS.
        use winit::platform::wayland::WindowAttributesExtWayland;
        attributes.with_name("graphite", "Graphite")
    };
    attributes
}
#[cfg(test)]
mod tests {
    #[test]
    fn default_window_has_the_embedded_graphite_icon() {
        let attributes = super::window_attributes();
        assert_eq!(attributes.title, "Graphite");
        assert!(attributes.window_icon.is_some());
    }
}
