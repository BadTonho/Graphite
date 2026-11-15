//! Placement of popups opened from a button of the activity bar.

/// Gap between a button and the popup that opens beside it.
const GAP: f32 = 4.0;

/// Top-left corner for a popup of `size` opened from `button` (`x, y, width, height`) in a
/// window of `window` size, whose bottom `bottom_inset` pixels belong to the status bar.
///
/// The popup sits beside the button, never over it: to its right when there is room and to its
/// left otherwise. A button in the lower part of the window gets a popup that ends level with
/// the button's bottom edge; a button near the top gets one that starts level with its top edge.
/// The popup is always kept inside the window above the status bar.
pub(crate) fn beside_button(
    button: (f32, f32, f32, f32),
    size: (f32, f32),
    window: (f32, f32),
    bottom_inset: f32,
) -> (f32, f32) {
    let (button_x, button_y, button_width, button_height) = button;
    let (width, height) = size;
    let (window_width, window_height) = window;
    let right = button_x + button_width + GAP;
    let x = if right + width <= window_width {
        right
    } else {
        (button_x - width - GAP).max(0.0)
    };
    let y = if button_y + button_height / 2.0 > window_height / 3.0 {
        button_y + button_height - height
    } else {
        button_y
    };
    let lowest = (window_height - bottom_inset - height).max(0.0);
    (x.min((window_width - width).max(0.0)), y.clamp(0.0, lowest))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: (f32, f32) = (1000.0, 800.0);

    #[test]
    fn a_popup_opens_to_the_right_of_a_bottom_left_button_ending_level_with_it() {
        let button = (4.0, 740.0, 28.0, 28.0);
        let (x, y) = beside_button(button, (290.0, 300.0), WINDOW, 20.0);
        assert_eq!(x, button.0 + button.2 + GAP);
        assert_eq!(y, button.1 + button.3 - 300.0);
        // Nothing of the popup overlaps the button.
        assert!(x >= button.0 + button.2);
    }

    #[test]
    fn a_popup_that_does_not_fit_on_the_right_opens_on_the_left() {
        let button = (960.0, 740.0, 28.0, 28.0);
        let (x, _) = beside_button(button, (290.0, 300.0), WINDOW, 20.0);
        assert_eq!(x, button.0 - 290.0 - GAP);
        assert!(x + 290.0 <= button.0);
    }

    #[test]
    fn a_button_near_the_top_gets_a_popup_that_starts_level_with_it() {
        let button = (4.0, 40.0, 28.0, 28.0);
        let (_, y) = beside_button(button, (290.0, 300.0), WINDOW, 20.0);
        assert_eq!(y, 40.0);
    }

    #[test]
    fn the_popup_stays_above_the_status_bar_and_inside_the_window() {
        let button = (4.0, 780.0, 28.0, 28.0);
        let (_, y) = beside_button(button, (290.0, 300.0), WINDOW, 20.0);
        assert!(y + 300.0 <= WINDOW.1 - 20.0);
        let (_, y) = beside_button((4.0, 0.0, 28.0, 28.0), (290.0, 300.0), WINDOW, 20.0);
        assert!(y >= 0.0);
        // A popup taller than the window still starts at the top.
        let (_, y) = beside_button(button, (290.0, 2000.0), WINDOW, 20.0);
        assert_eq!(y, 0.0);
    }
}
