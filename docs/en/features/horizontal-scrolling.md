# Horizontal scrolling

An editor view that does not wrap its lines scrolls sideways when a line is wider than its text area. The text area is the group's width without the line-number column and the minimap, so it changes whenever the group is resized, for example when the secondary side bar is dragged wider or opened.

- **Following the caret:** moving or typing past the right or left edge scrolls the text just enough to bring the caret back into view. Tabs count with their expanded width.
- **Following the group:** the scroll never rests past the end of the longest visible line. When the group gets wider (a side bar closed or dragged narrower) the text comes back into view at once.
- **Mouse:** a sideways swipe on a touchpad, or the mouse wheel with `Shift` held, scrolls sideways. Clicks and drags map to the scrolled text.
- **Line numbers stay put:** the line-number column, its markers and the sticky scroll headers do not scroll sideways; the text is clipped to the area right of the column.
- **Word wrap:** with [word wrap](word-wrap.md) on for a view, its lines wrap to the text area instead and the sideways scroll is always zero.

The scroll is stored per tab (`View::scroll_col`, in columns). The rules live in `app/editing/horizontal_scroll.rs` and are covered by its unit tests; the drawing is in `app/editing/group_drawing.rs`.
