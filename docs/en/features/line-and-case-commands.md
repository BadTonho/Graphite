# Line and case commands

Text commands that rewrite whole lines or the case of words. All of them are undone in one step, need a writable editor, and are available in the command palette.

## Lines

| Command | Effect |
| --- | --- |
| **Indent line** (`Ctrl+]`) | Moves each selected line to the next indent stop, using the document's tab size and spaces/tabs setting. Empty lines are skipped when several lines are selected. A cursor on a blank line ends up after the new indentation. |
| **Outdent line** (`Ctrl+[`) | Moves each selected line to the previous indent stop. Lines without leading whitespace are left alone. |
| **Reverse lines** | Reverses the order of the selected lines. With a lone cursor it reverses the whole document, without the empty line after the final line break. Each cursor keeps its column and follows its line. |
| **Sort lines descending** | Sorts the selected lines in descending order. With a lone cursor it sorts the whole document. |
| **Sort lines ascending** | Same as above, ascending. A lone cursor now sorts the whole document too. |
| **Duplicate selection** | Copies each selection right after itself and selects the copy; a cursor without a selection duplicates its line below. |
| **Transpose characters around the cursor** | Swaps the characters before and after the cursor. At the end of a line it swaps the last character with the line break, moving that character to the start of the next line. Does nothing at the start of a line or at the end of the text. |

Line separators (`\n` or `\r\n`) stay where they were; only the text of the lines moves.

## Case

These commands act on each selection, or on the word at the cursor when nothing is selected. A word is made of letters, digits and underscores.

| Command | Example |
| --- | --- |
| **Transform to uppercase** | `Straße` → `STRASSE` |
| **Transform to lowercase** | `HeLLo` → `hello` |
| **Transform to title case** | `don't stop` → `Don't Stop` |
| **Transform to snake case** | `parseHTTPResponse` → `parse_http_response` |
| **Transform to camel case** | `hello_world-foo bar` → `helloWorldFooBar` |
| **Transform to Pascal case** | `HELLO_WORLD` → `HelloWorld` |
| **Transform to kebab case** | `parseHTTPResponse` → `parse-http-response` |

Details:

- Camel case keeps spaces when the selection spans several lines, and leaves a leading acronym (`HTTP server` → `HTTPServer`) alone.
- Pascal case keeps a `.` where it is and lower-cases the tail of all-capital words.
- Kebab case turns an underscore between two non-blank characters into a hyphen. Matches do not overlap, so in `a_b_c` only the first underscore changes (`a-b_c`).
- Two cursors in the same word change it once. A cursor that touches no word does nothing.
