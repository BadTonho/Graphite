use std::fmt::Display;

/// Replaces `{name}` placeholders with the matching argument.
///
/// Unknown placeholders are preserved so missing translations are visible
/// instead of silently dropping data. `{{` and `}}` produce literal braces.
pub fn interpolate(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find(['{', '}']) {
        result.push_str(&rest[..open]);
        rest = &rest[open..];
        let mut chars = rest.chars();
        let marker = chars.next().unwrap_or_default();
        if rest[marker.len_utf8()..].starts_with(marker) {
            result.push(marker);
            rest = &rest[marker.len_utf8() * 2..];
            continue;
        }
        if marker == '}' {
            result.push('}');
            rest = &rest[1..];
            continue;
        }
        let Some(end) = rest[1..].find('}') else {
            result.push_str(rest);
            rest = "";
            break;
        };
        let name = &rest[1..1 + end];
        if let Some((_, value)) = args.iter().find(|(candidate, _)| *candidate == name) {
            use std::fmt::Write as _;
            let _ = write!(result, "{value}");
        } else {
            result.push('{');
            result.push_str(name);
            result.push('}');
        }
        rest = &rest[1 + end + 1..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_repeated_placeholders() {
        assert_eq!(
            interpolate("{a}-{b}-{a}", &[("a", &1), ("b", &"x")]),
            "1-x-1"
        );
    }

    #[test]
    fn supports_escaped_braces() {
        assert_eq!(
            interpolate("{{name}} {name} }}", &[("name", &"ok")]),
            "{name} ok }"
        );
    }

    #[test]
    fn leaves_unknown_and_unclosed_placeholders_visible() {
        assert_eq!(interpolate("Hi {name}", &[]), "Hi {name}");
        assert_eq!(interpolate("Hi {name", &[("name", &"x")]), "Hi {name");
    }
}
