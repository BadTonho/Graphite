# Keyboard focus between regions and high contrast

## Moving focus with F6

`F6` moves keyboard focus through the main regions of the window, in this order:

1. The editor.
2. The terminal, when the panel is visible.
3. The explorer, when the primary sidebar is visible.

Regions that are hidden are skipped. `Shift + F6` moves in the opposite direction. The status bar says where focus went. Clicking a region also moves focus there.

`F6` used to move between editor groups. Group navigation is still available from the command palette (`Focus next group`) and its own shortcuts.

## High contrast

Run **Toggle high contrast** from the command palette. The setting is saved in the profile (`high-contrast.json`) and restored at startup.

High contrast uses the dark palette:

- Dark surfaces become pure black and light text becomes pure white.
- Mid-tone colors (syntax colors, dim text) are kept.
- The active row (current selection) becomes a saturated blue so it stays visible.

Limits: some areas use fixed colors that are not passed through this mapping yet, such as the terminal background. The palette has not been checked visually for every screen.

## Not covered here

Screen reader support (through the accessibility tree) and input method (IME) behavior need manual checks with real assistive technology and real input methods. They are not verified by automated tests.
