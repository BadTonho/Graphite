use graphite_i18n::Translator;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expr {
    Key(String),
    Compare(String, String, bool),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    pub(crate) fn parse(source: &str) -> Result<Self, String> {
        parse(source)
    }

    pub(crate) fn evaluate(&self, context: &HashMap<String, String>) -> bool {
        match self {
            Self::Key(key) => match key.as_str() {
                "true" => true,
                "false" => false,
                _ => context
                    .get(key)
                    .is_some_and(|value| !value.is_empty() && value != "false" && value != "0"),
            },
            Self::Compare(key, expected, equal) => {
                (context.get(key).map(String::as_str).unwrap_or("") == expected) == *equal
            }
            Self::Not(expr) => !expr.evaluate(context),
            Self::And(left, right) => left.evaluate(context) && right.evaluate(context),
            Self::Or(left, right) => left.evaluate(context) || right.evaluate(context),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Atom(String),
    Not,
    And,
    Or,
    Eq,
    Ne,
    Left,
    Right,
}

pub(super) fn parse(source: &str) -> Result<Expr, String> {
    parse_with(source, &Translator::english())
}

pub(super) fn parse_with(source: &str, translator: &Translator) -> Result<Expr, String> {
    let tokens = tokenize(source, translator)?;
    let mut parser = Parser {
        tokens,
        index: 0,
        translator,
    };
    let expr = parser.or()?;
    if parser.index != parser.tokens.len() {
        return Err(translator.text("config.when.unexpected_token").into());
    }
    Ok(expr)
}

fn tokenize(source: &str, translator: &Translator) -> Result<Vec<Token>, String> {
    let chars: Vec<_> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let next = chars.get(i + 1).copied();
        let pair = match (chars[i], next) {
            ('&', Some('&')) => Some(Token::And),
            ('|', Some('|')) => Some(Token::Or),
            ('=', Some('=')) => Some(Token::Eq),
            ('!', Some('=')) => Some(Token::Ne),
            _ => None,
        };
        if let Some(token) = pair {
            out.push(token);
            i += 2;
            continue;
        }
        match chars[i] {
            '!' => {
                out.push(Token::Not);
                i += 1;
            }
            '(' => {
                out.push(Token::Left);
                i += 1;
            }
            ')' => {
                out.push(Token::Right);
                i += 1;
            }
            '\'' | '"' => {
                let quote = chars[i];
                i += 1;
                let mut value = String::new();
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == quote {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    value.push(chars[i]);
                    i += 1;
                }
                if !closed {
                    return Err(translator.text("config.when.unterminated_string").into());
                }
                out.push(Token::Atom(value));
            }
            c if c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '/' | ':') => {
                let start = i;
                i += 1;
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || matches!(chars[i], '_' | '.' | '-' | '/' | ':'))
                {
                    i += 1;
                }
                out.push(Token::Atom(chars[start..i].iter().collect()));
            }
            other => {
                return Err(translator.format(
                    "config.when.unsupported_character",
                    &[("character", &other)],
                ))
            }
        }
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    index: usize,
    translator: &'a Translator,
}
impl Parser<'_> {
    fn or(&mut self) -> Result<Expr, String> {
        let mut value = self.and()?;
        while self.consume(&Token::Or) {
            value = Expr::Or(Box::new(value), Box::new(self.and()?));
        }
        Ok(value)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut value = self.primary()?;
        while self.consume(&Token::And) {
            value = Expr::And(Box::new(value), Box::new(self.primary()?));
        }
        Ok(value)
    }
    fn primary(&mut self) -> Result<Expr, String> {
        if self.consume(&Token::Not) {
            return Ok(Expr::Not(Box::new(self.primary()?)));
        }
        if self.consume(&Token::Left) {
            let value = self.or()?;
            if !self.consume(&Token::Right) {
                return Err(self.translator.text("config.when.missing_paren").into());
            }
            return Ok(value);
        }
        let Some(Token::Atom(key)) = self.tokens.get(self.index).cloned() else {
            return Err(self.translator.text("config.when.expected_key").into());
        };
        self.index += 1;
        let equal = if self.consume(&Token::Eq) {
            Some(true)
        } else if self.consume(&Token::Ne) {
            Some(false)
        } else {
            None
        };
        if let Some(equal) = equal {
            let Some(Token::Atom(value)) = self.tokens.get(self.index).cloned() else {
                return Err(self.translator.text("config.when.expected_value").into());
            };
            self.index += 1;
            Ok(Expr::Compare(key, value, equal))
        } else {
            Ok(Expr::Key(key))
        }
    }
    fn consume(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.index) == Some(token) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expression_precedence_and_comparisons_follow_context() {
        let expression =
            parse("editorFocus && !editorReadonly && (lang == 'rust' || lang == 'python')")
                .unwrap();
        let mut context = HashMap::from([
            ("editorFocus".into(), "true".into()),
            ("lang".into(), "rust".into()),
        ]);
        assert!(expression.evaluate(&context));
        context.insert("editorReadonly".into(), "true".into());
        assert!(!expression.evaluate(&context));
        assert!(parse("focus && (").is_err());
        assert!(parse("lang =~ /rust/").is_err());
        assert!(parse("true && !false").unwrap().evaluate(&HashMap::new()));
    }
}
