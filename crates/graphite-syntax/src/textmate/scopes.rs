//! Mapping TextMate scopes onto the highlight classes themes color.
//!
//! A theme gives a color to the most specific scope rule that matches a token. Graphite's themes
//! color [`Class`]es, so a token takes the class of its innermost scope that has one; scopes
//! themes leave uncolored (punctuation, plain `meta`) pass the choice outward, which is why the
//! quotes of a string are colored like the string.

use crate::Class;

/// Scope prefixes and the class they stand for. The longest matching prefix of a scope wins.
const TABLE: &[(&str, Class)] = &[
    ("comment", Class::Comment),
    ("string", Class::String),
    ("string.regexp", Class::String),
    ("constant.numeric", Class::Number),
    ("constant.character.escape", Class::Constant),
    ("constant.character", Class::Constant),
    ("constant.language", Class::Constant),
    ("constant.other", Class::Constant),
    ("constant", Class::Constant),
    ("keyword.operator", Class::Operator),
    ("keyword.other.unit", Class::Number),
    ("keyword", Class::Keyword),
    ("storage", Class::Keyword),
    ("entity.name.function", Class::Function),
    ("entity.name.method", Class::Function),
    ("entity.name.command", Class::Function),
    ("entity.name.type", Class::Type),
    ("entity.name.class", Class::Type),
    ("entity.name.struct", Class::Type),
    ("entity.name.enum", Class::Type),
    ("entity.name.interface", Class::Type),
    ("entity.name.namespace", Class::Type),
    ("entity.name.module", Class::Type),
    ("entity.name.tag", Class::Type),
    ("entity.name.section", Class::Keyword),
    ("entity.other.inherited-class", Class::Type),
    ("entity.other.attribute-name", Class::Property),
    ("support.function", Class::Function),
    ("support.type.property-name", Class::Property),
    ("support.type", Class::Type),
    ("support.class", Class::Type),
    ("support.constant", Class::Constant),
    ("support.variable", Class::Variable),
    ("meta.function-call", Class::Function),
    ("meta.object-literal.key", Class::Property),
    ("meta.preprocessor", Class::Keyword),
    ("variable.parameter", Class::Variable),
    ("variable.language", Class::Constant),
    ("variable.other.constant", Class::Constant),
    ("variable.other.property", Class::Property),
    ("variable.other.object.property", Class::Property),
    ("variable.object.property", Class::Property),
    ("variable", Class::Variable),
    ("markup.heading", Class::Keyword),
    ("markup.bold", Class::Constant),
    ("markup.italic", Class::String),
    ("markup.inline.raw", Class::String),
    ("markup.raw", Class::String),
    ("markup.underline.link", Class::Function),
    ("markup.list", Class::Operator),
    ("markup.quote", Class::Comment),
    ("invalid", Class::Constant),
];

/// The class of a single scope, if themes color it.
fn class_of(scope: &str) -> Option<Class> {
    TABLE
        .iter()
        .filter(|(prefix, _)| {
            scope == *prefix
                || (scope.starts_with(prefix) && scope.as_bytes().get(prefix.len()) == Some(&b'.'))
        })
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, class)| *class)
}

/// The class of a token with the scopes `scopes`, from outermost to innermost.
pub fn class_for_scopes<S: AsRef<str>>(scopes: &[S]) -> Class {
    scopes
        .iter()
        .rev()
        .find_map(|scope| class_of(scope.as_ref()))
        .unwrap_or(Class::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_innermost_colored_scope_decides() {
        assert_eq!(class_for_scopes(&["source.x", "string.quoted.double.x"]), Class::String);
        // Quotes are punctuation inside the string and take the string's color.
        assert_eq!(
            class_for_scopes(&["source.x", "string.quoted.double.x", "punctuation.definition.string.begin.x"]),
            Class::String
        );
        assert_eq!(
            class_for_scopes(&["source.x", "string.quoted", "constant.character.escape.x"]),
            Class::Constant
        );
        assert_eq!(class_for_scopes(&["source.x", "keyword.operator.assignment.x"]), Class::Operator);
        assert_eq!(class_for_scopes(&["source.x", "keyword.control.x"]), Class::Keyword);
        assert_eq!(class_for_scopes(&["source.x", "constant.numeric.integer.x"]), Class::Number);
        assert_eq!(class_for_scopes(&["source.x", "variable.other.property.x"]), Class::Property);
        assert_eq!(class_for_scopes(&["source.x", "entity.name.function.x"]), Class::Function);
        assert_eq!(class_for_scopes(&["source.x", "meta.block.x"]), Class::Default);
        assert_eq!(class_for_scopes::<&str>(&[]), Class::Default);
    }

    #[test]
    fn a_prefix_must_end_at_a_dot() {
        assert_eq!(class_of("keywordish"), None);
        assert_eq!(class_of("stringy.thing"), None);
        assert_eq!(class_of("string"), Some(Class::String));
        assert_eq!(class_of("keyword.control.flow"), Some(Class::Keyword));
    }
}
