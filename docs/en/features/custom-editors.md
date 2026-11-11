# Custom read-only views

Some files cannot be edited as plain text. Graphite opens them in a **read-only view** instead of failing:

| File | View |
|---|---|
| Binary, or not UTF-8 text | A hex dump: offset, 16 bytes in hex, and an ASCII column. Files over 1 MiB show the first 1 MiB and say how many bytes were left out. |
| Jupyter notebook (`.ipynb`) | The cells in order. Each cell has a header (`---- cell N (code) ----`), its source, and its outputs (stream text, `text/plain` results, and errors). |

Views are read-only documents. They can be selected, searched, and copied, but they cannot be saved, so the original file is never overwritten with the rendered text. Files changed on disk are not reloaded into a view, because its text is not the file's contents.

Notebooks are shown read-only only. Editing cells, running them, and rich outputs (images, HTML) are not supported yet.
