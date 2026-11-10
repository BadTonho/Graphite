use super::*;

/// Expands the supported Emmet core for HTML/JSX and common CSS property snippets.
pub(crate) fn expand(abbreviation: &str, language: &str) -> Option<String> {
    if language == "css" {
        return Some(match abbreviation {
            "m" => "margin: ;".into(),
            "m10" => "margin: 10px;".into(),
            "p" => "padding: ;".into(),
            "p10" => "padding: 10px;".into(),
            "d:f" => "display: flex;".into(),
            "d:g" => "display: grid;".into(),
            "w100p" => "width: 100%;".into(),
            _ => return None,
        });
    }
    if !["html", "jsx", "javascript"].contains(&language) {
        return None;
    }
    let mut levels = abbreviation.split('>').collect::<Vec<_>>();
    if levels.is_empty() || levels.iter().any(|part| part.is_empty()) {
        return None;
    }
    let leaf = levels.pop()?;
    let (leaf_tag, leaf_attrs, leaf_text, leaf_count) = parse_element(leaf)?;
    let mut content = render_element(&leaf_tag, &leaf_attrs, leaf_text.as_deref(), leaf_count);
    while let Some(parent) = levels.pop() {
        let (tag, attrs, text, count) = parse_element(parent)?;
        let nested = indent_block(&content, 1);
        let one = format!("<{}{}>\n{}\n</{}>", tag, attrs, nested, tag);
        content = if count > 1 {
            std::iter::repeat_n(one, count)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            one
        };
        if let Some(text) = text {
            content = content.replace(&format!("</{}>", tag), &format!("{text}</{}>", tag));
        }
    }
    Some(content)
}
fn parse_element(input: &str) -> Option<(String, String, Option<String>, usize)> {
    let (without_text, text) = if let Some((before, after)) = input.split_once('{') {
        (before, Some(after.strip_suffix('}')?.to_owned()))
    } else {
        (input, None)
    };
    let (selector, count) = if let Some((s, n)) = without_text.rsplit_once('*') {
        (s, n.parse::<usize>().ok()?)
    } else {
        (without_text, 1)
    };
    if count == 0 || count > 100 {
        return None;
    }
    let mut pieces = selector.split(['.', '#']);
    let first = pieces.next()?;
    let tag = if first.is_empty() { "div" } else { first };
    if !tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    let mut id = None;
    let mut classes = Vec::new();
    let mut rest = selector.strip_prefix(first)?;
    while !rest.is_empty() {
        let kind = rest.chars().next()?;
        rest = &rest[1..];
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let value = &rest[..end];
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return None;
        }
        if kind == '#' {
            if id.replace(value).is_some() {
                return None;
            }
        } else {
            classes.push(value);
        }
        rest = &rest[end..];
    }
    let mut attrs = String::new();
    if let Some(id) = id {
        attrs.push_str(&format!(" id=\"{id}\""));
    }
    if !classes.is_empty() {
        attrs.push_str(&format!(" class=\"{}\"", classes.join(" ")));
    }
    Some((tag.into(), attrs, text, count))
}
fn render_element(tag: &str, attrs: &str, text: Option<&str>, count: usize) -> String {
    let one = format!("<{}{}>{}</{}>", tag, attrs, text.unwrap_or(""), tag);
    std::iter::repeat_n(one, count)
        .collect::<Vec<_>>()
        .join("\n")
}
fn indent_block(block: &str, depth: usize) -> String {
    let prefix = "  ".repeat(depth);
    block
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl App {
    pub(crate) fn expand_emmet_at_cursor(&mut self) -> bool {
        let path = self.documents[self.command_document()]
            .buffer
            .path()
            .map(|p| p.to_path_buf());
        let Some(path) = path else {
            return false;
        };
        let language = match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "html" | "htm" => "html",
            "jsx" => "jsx",
            "js" | "mjs" => "javascript",
            "css" => "css",
            _ => return false,
        };
        let editor = self.editor();
        let cursor = editor.cursor().char_idx;
        let text = editor.text();
        let before = text.chars().take(cursor).collect::<String>();
        let start = before
            .char_indices()
            .rev()
            .find(|(_, ch)| ch.is_whitespace())
            .map_or(0, |(idx, _)| before[..idx].chars().count() + 1);
        let abbreviation = &before[start..];
        let Some(expanded) = expand(abbreviation, language) else {
            return false;
        };
        let end = cursor;
        if self
            .editor_mut()
            .apply_transaction(&[graphite_text::TextReplacement {
                range: start..end,
                text: expanded,
            }])
            .is_err()
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_nested_html_jsx_and_css_abbreviations_and_rejects_invalid_input() {
        assert_eq!(
            expand("ul>li*2", "html").unwrap(),
            "<ul>\n  <li></li>\n  <li></li>\n</ul>"
        );
        assert_eq!(
            expand("div.card#main{hello}", "jsx").unwrap(),
            "<div id=\"main\" class=\"card\">hello</div>"
        );
        assert_eq!(expand("d:f", "css").unwrap(), "display: flex;");
        assert!(expand("div>???", "html").is_none());
        assert!(expand("ul", "rust").is_none());
    }
}
