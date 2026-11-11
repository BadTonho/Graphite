# Terminal: shell integration and links

## Prompts, exit codes and history

Graphite reads the standard shell integration sequences from the terminal output:

- **OSC 133** marks each prompt and reports the exit code of the command that just finished.
- **OSC 7** reports the shell's working directory.

When a command finishes, a bar in the left margin of the terminal shows its result: green for exit code 0, red for any other code, grey when the shell did not report one.

| Action | Keys |
|---|---|
| Go to previous prompt | `Ctrl + Up` (terminal focused) |
| Go to next prompt | `Ctrl + Down` (terminal focused) |

The prompt jumps keep the terminal's scrollback. Up to 1000 marks are kept per terminal.

### Shells that Graphite prepares

- **PowerShell** (`pwsh` and Windows PowerShell): Graphite starts it with `-NoExit -Command <prompt wrapper>`. The wrapper keeps your prompt text, adds the sequences, and runs first. A profile that already passes `-Command`, `-c` or `-File` is left as you configured it.
- **bash** (including Git Bash): Graphite sets `PROMPT_COMMAND`. A `~/.bashrc` that assigns `PROMPT_COMMAND` itself replaces this, so the integration stops working for that shell.

zsh and fish are not prepared yet. Their prompts still work, but no marks, exit-code bars or prompt jumps appear.

The working directory from OSC 7 is stored with the terminal session and is used to resolve relative file links. It is not shown in the panel yet.

## Links

Hold `Ctrl` and click text in the terminal output:

- An `https://` URL opens in the default browser. Other schemes are not opened.
- A `path:line` or `path:line:column` reference opens the file in the editor at that position. Relative paths are resolved from the shell's working directory (OSC 7) when known, otherwise from the workspace root. Windows drive letters are supported.

Links are recognized on the line under the pointer, so a link that wraps onto the next line is only partly recognized.

## Colors

Text colors use the terminal's palette, 256-color indexes, and 24-bit TrueColor sequences (`ESC[38;2;r;g;bm` and `ESC[48;2;r;g;bm`). The terminal is drawn by Graphite's GPU renderer.
