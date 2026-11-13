//! Case transformations applied to selected text or the word at the cursor.

/// A way of rewriting the case and word separators of a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseTransform {
    Upper,
    Lower,
    Title,
    Snake,
    Camel,
    Pascal,
    Kebab,
}

impl CaseTransform {
    pub fn apply(self, text: &str) -> String {
        match self {
            CaseTransform::Upper => text.to_uppercase(),
            CaseTransform::Lower => text.to_lowercase(),
            CaseTransform::Title => title_case(text),
            CaseTransform::Snake => snake_case(text),
            CaseTransform::Camel => camel_case(text),
            CaseTransform::Pascal => pascal_case(text),
            CaseTransform::Kebab => kebab_case(text),
        }
    }
}

fn upper(c: char) -> bool {
    c.is_uppercase()
}

fn lower(c: char) -> bool {
    c.is_lowercase()
}

/// Upper-cases the first letter of every word; the rest of the text is lower-cased. A letter
/// after an apostrophe that follows a word ("don't") does not start a word.
fn title_case(text: &str) -> String {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &c) in chars.iter().enumerate() {
        let starts_word = c.is_alphabetic()
            && match index.checked_sub(1).map(|p| chars[p]) {
                None => true,
                Some('\'') => index < 2 || !chars[index - 2].is_alphabetic(),
                Some(previous) => !previous.is_alphanumeric(),
            };
        if starts_word {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Inserts `separator` between a lower-case letter and an upper-case one, and before the last
/// capital of a run of capitals that is followed by a lower-case letter ("HTTPServer").
fn split_words(text: &str, separator: char) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut first = String::with_capacity(text.len() + 4);
    let mut index = 0;
    while index < chars.len() {
        first.push(chars[index]);
        if index + 1 < chars.len() && lower(chars[index]) && upper(chars[index + 1]) {
            first.push(separator);
        }
        index += 1;
    }
    let chars: Vec<char> = first.chars().collect();
    let mut out = String::with_capacity(first.len() + 4);
    let mut index = 0;
    while index < chars.len() {
        if index + 2 < chars.len()
            && (upper(chars[index]) || chars[index].is_numeric())
            && upper(chars[index + 1])
            && lower(chars[index + 2])
        {
            out.push(chars[index]);
            out.push(separator);
            out.push(chars[index + 1]);
            out.push(chars[index + 2]);
            index += 3;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn snake_case(text: &str) -> String {
    split_words(text, '_').to_lowercase()
}

/// Kebab case first turns `_` between two non-blank characters into `-`. Matches do not
/// overlap, so in `a_b_c` only the first underscore is replaced.
fn kebab_case(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut joined = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if index + 2 < chars.len()
            && !chars[index].is_whitespace()
            && chars[index + 1] == '_'
            && !chars[index + 2].is_whitespace()
        {
            joined.push(chars[index]);
            joined.push('-');
            joined.push(chars[index + 2]);
            index += 3;
        } else {
            joined.push(chars[index]);
            index += 1;
        }
    }
    split_words(&joined, '-').to_lowercase()
}

/// Splits on runs of separator characters, keeping the empty pieces at either end.
fn split_runs(text: &str, is_separator: impl Fn(char) -> bool) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut in_run = false;
    for c in text.chars() {
        if is_separator(c) {
            if !in_run {
                parts.push(String::new());
                in_run = true;
            }
        } else {
            in_run = false;
            parts.last_mut().expect("never empty").push(c);
        }
    }
    parts
}

fn camel_case(text: &str) -> String {
    let multiline = text.contains(['\r', '\n']);
    let parts = if multiline {
        split_runs(text, |c| c == '_' || c == '-')
    } else {
        split_runs(text, |c| c == '_' || c == '-' || c.is_whitespace())
    };
    let mut parts = parts.into_iter();
    let first = lowercase_line_starts(&parts.next().unwrap_or_default());
    let mut out = first;
    for word in parts {
        let mut chars = word.chars();
        if let Some(head) = chars.next() {
            out.extend(head.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// A capital that starts a line (or the text) and is followed by a character that is not a
/// capital is lower-cased, so `Hello` becomes `hello` but `HTTP` is kept.
fn lowercase_line_starts(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(word.len());
    let mut index = 0;
    while index < chars.len() {
        let line_start = index == 0 || chars[index - 1] == '\n';
        if line_start && index + 1 < chars.len() && upper(chars[index]) && !upper(chars[index + 1])
        {
            out.extend(chars[index].to_lowercase());
            out.extend(chars[index + 1].to_lowercase());
            index += 2;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn pascal_case(text: &str) -> String {
    // Pieces end after each `.`, which is kept, then split on `_`, space, tab and `-`.
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if c == '.' {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() || pieces.is_empty() {
        pieces.push(current);
    }
    let mut out = String::new();
    for piece in pieces {
        for word in piece.split(['_', ' ', '\t', '-']) {
            let mut chars = word.chars();
            let Some(head) = chars.next() else { continue };
            let mut normalized: String = head.to_uppercase().collect();
            normalized.push_str(chars.as_str());
            let all_caps = normalized.chars().count() > 1
                && normalized
                    .split(['\n', '\r'])
                    .any(|line| !line.is_empty() && line.chars().all(upper));
            if all_caps {
                let mut chars = normalized.chars();
                if let Some(head) = chars.next() {
                    out.push(head);
                    out.push_str(&chars.as_str().to_lowercase());
                }
            } else {
                out.push_str(&normalized);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::CaseTransform::*;

    #[test]
    fn upper_and_lower_change_every_letter() {
        assert_eq!(Upper.apply("Straße ok"), "STRASSE OK");
        assert_eq!(Lower.apply("HeLLo"), "hello");
    }

    #[test]
    fn title_case_capitalizes_words_and_keeps_apostrophes_inside_words() {
        assert_eq!(Title.apply("hELLO wORLD"), "Hello World");
        assert_eq!(Title.apply("don't stop"), "Don't Stop");
        assert_eq!(Title.apply("'quoted' text"), "'Quoted' Text");
        assert_eq!(Title.apply("one-two\nthree"), "One-Two\nThree");
        assert_eq!(Title.apply("área útil"), "Área Útil");
        assert_eq!(Title.apply("route66 go"), "Route66 Go");
    }

    #[test]
    fn snake_case_splits_camel_words_and_acronyms() {
        assert_eq!(Snake.apply("helloWorld"), "hello_world");
        assert_eq!(Snake.apply("HelloWorld"), "hello_world");
        assert_eq!(Snake.apply("parseHTTPResponse"), "parse_http_response");
        assert_eq!(Snake.apply("already_snake"), "already_snake");
        assert_eq!(Snake.apply("a1Bc"), "a1_bc");
    }

    #[test]
    fn camel_case_joins_words_and_lowers_the_first_one() {
        assert_eq!(Camel.apply("hello_world-foo bar"), "helloWorldFooBar");
        assert_eq!(Camel.apply("Hello World"), "helloWorld");
        assert_eq!(Camel.apply("HTTP server"), "HTTPServer");
        assert_eq!(Camel.apply("_private_name"), "PrivateName");
        // Spaces are kept when the text spans several lines.
        assert_eq!(Camel.apply("one_two\nthree_four"), "oneTwo\nthreeFour");
    }

    #[test]
    fn pascal_case_capitalizes_every_word_and_tames_shouting() {
        assert_eq!(Pascal.apply("hello_world-foo bar"), "HelloWorldFooBar");
        assert_eq!(Pascal.apply("HELLO_WORLD"), "HelloWorld");
        assert_eq!(Pascal.apply("a.b_c"), "A.BC");
        assert_eq!(Pascal.apply("x"), "X");
    }

    #[test]
    fn kebab_case_splits_words_with_hyphens() {
        assert_eq!(Kebab.apply("helloWorld"), "hello-world");
        assert_eq!(Kebab.apply("snake_case"), "snake-case");
        assert_eq!(Kebab.apply("parseHTTPResponse"), "parse-http-response");
        // Underscore replacements do not overlap, so alternating ones are converted pairwise.
        assert_eq!(Kebab.apply("a_b_c"), "a-b_c");
    }
}
