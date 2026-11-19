# Word wrap

Whether an editor view wraps its long lines is decided for each view, from three things, in this order:

1. **Your choice for that view.** View → Word Wrap (`Alt+Z`) flips the wrapping of the active editor only, and keeps the text that is at the top of the view at the top. It is not saved and does not touch the other editors.
2. **The `editor.wordWrap` setting**, when you or the workspace set it to `on` or `off`. It applies to every file. Changing the setting replaces earlier per-view choices.
3. **The default for the kind of file:** Markdown and plain text wrap, code does not.

So prose reads comfortably without any setup, while code keeps its long lines and its code folding.

## Which files wrap by default

- Markdown: `.md`, `.markdown`, `.mdx`.
- Plain text: new files that have not been saved yet, `.txt` and `.text`, and the usual prose files that have no extension (`README`, `LICENSE`, `NOTICE`, `AUTHORS`, `COPYING`, `CHANGELOG`, `CONTRIBUTING`).
- Everything else, including every file with a known programming language and data files such as `.csv`, does not wrap.

## Notes

- With wrapping on, folding is unavailable in that view and the view does not scroll sideways (see [Horizontal scrolling](horizontal-scrolling.md)); with it off, a long line scrolls sideways.
- The default is decided from the file's name and language, so saving a new file as `notes.md` or `main.rs` changes its default.
- Language-specific settings (an `editor.wordWrap` that applies to one language only) are not supported yet.
