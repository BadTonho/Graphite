# Workbench layout

The window is made of four blocks that tile without overlapping: the primary side bar, the editor column (the editor groups with the panel below them), and the secondary side bar. The activity bar sits beside them. Changing the size of one block makes its neighbours follow.

## The editor column

The editor groups and the panel share one column. Its left and right edges are defined once (`editor_column` in `app/window/layout.rs`) and every consumer reads them: the editor groups, the panel (terminal, problems, debug console and the like), the terminal's column count and the main draw routine. The panel never runs under a side bar, whichever side the primary side bar is on, in zen mode, and with a panel placed to the right of the editor. An aligned panel is aligned within the column.

## Sizes

- Each side bar has a minimum width, and the editor column keeps at least 320 px.
- The block you are dragging wins the space and its neighbour gives way. Dragging the secondary side bar wider narrows the primary one, down to its minimum; dragging the primary side bar wider narrows the secondary one, down to its minimum. Neither can take the editor below its minimum.
- The sizes you set are kept: when the other block shrinks again, the one that gave way comes back to what it had.

## Dividers

There are three draggable dividers: primary side bar | editor column, editor groups | panel, and editor column | secondary side bar. Each one lights up in the accent color while the pointer is over it or it is being dragged, and a double click puts its block back to the default size (the secondary side bar also forgets its saved width). The secondary side bar has a thin border on the edge that faces the editor, like the other blocks.

The widths of both side bars are saved per profile.
