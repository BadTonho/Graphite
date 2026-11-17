//! TextMate grammars: parsing their JSON (or plist) form into rules, and translating their
//! Oniguruma regular expressions for the `fancy-regex` engine.

use fancy_regex::Regex;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

/// A regular expression of a rule. One that does not compile (an Oniguruma feature the engine
/// lacks) never matches, so the rest of the grammar still works.
pub(super) struct Pattern {
    regex: Option<Regex>,
    /// The pattern contains `\G`: it only matches where the previous match ended.
    pub anchored: bool,
}

/// The groups of a match as byte ranges of the text searched.
pub(super) type Groups = Vec<Option<(usize, usize)>>;

impl Pattern {
    pub(super) fn new(source: &str) -> Self {
        let (translated, anchored) = translate(source);
        Self {
            regex: Regex::new(&translated).ok(),
            anchored,
        }
    }

    /// The first match at or after `from`, as the ranges of its groups. A `\G` pattern matches
    /// only at `anchor`.
    pub(super) fn find(&self, text: &str, from: usize, anchor: Option<usize>) -> Option<Groups> {
        let regex = self.regex.as_ref()?;
        let captures = regex.captures_from_pos(text, from).ok()??;
        let whole = captures.get(0)?;
        if self.anchored && anchor != Some(whole.start()) {
            return None;
        }
        Some(
            (0..captures.len())
                .map(|index| {
                    captures
                        .get(index)
                        .map(|group| (group.start(), group.end()))
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(super) fn compiles(&self) -> bool {
        self.regex.is_some()
    }
}

/// Translates an Oniguruma pattern. Returns the pattern for `fancy-regex` and whether it used
/// `\G`. Lines are matched one at a time, so `^` and `$` are line anchors.
pub(super) fn translate(source: &str) -> (String, bool) {
    let mut out = String::from("(?m)");
    let mut anchored = false;
    let mut in_class = 0usize;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some('G') if in_class == 0 => anchored = true,
                Some('h') => out.push_str(if in_class > 0 {
                    "0-9a-fA-F"
                } else {
                    "[0-9a-fA-F]"
                }),
                Some('H') if in_class == 0 => out.push_str("[^0-9a-fA-F]"),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push_str("\\\\"),
            },
            '[' => {
                in_class += 1;
                out.push('[');
                // A `]` right after the opening is a literal member.
                if chars.peek() == Some(&'^') {
                    out.push('^');
                    chars.next();
                }
                if chars.peek() == Some(&']') {
                    out.push_str("\\]");
                    chars.next();
                }
            }
            ']' if in_class > 0 => {
                in_class -= 1;
                out.push(']');
            }
            other => out.push(other),
        }
    }
    (out, anchored)
}

/// The end pattern of a `begin`/`end` rule. It may refer to the groups of the begin match.
pub(super) struct EndPattern {
    pub source: String,
    pub has_backrefs: bool,
}

impl EndPattern {
    fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let has_backrefs = bytes
            .windows(2)
            .any(|pair| pair[0] == b'\\' && pair[1].is_ascii_digit());
        Self {
            source: source.to_owned(),
            has_backrefs,
        }
    }

    /// The pattern with `\1`.. replaced by the text of those groups of the begin match.
    pub(super) fn resolve(&self, line: &str, groups: &Groups) -> String {
        if !self.has_backrefs {
            return self.source.clone();
        }
        let mut out = String::new();
        let mut chars = self.source.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.peek().copied() {
                    Some(digit) if digit.is_ascii_digit() => {
                        chars.next();
                        let index = digit.to_digit(10).unwrap_or(0) as usize;
                        if let Some(Some((start, end))) = groups.get(index) {
                            out.push_str(&fancy_regex::escape(&line[*start..*end]));
                        }
                    }
                    Some(other) => {
                        chars.next();
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push_str("\\\\"),
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Include {
    /// `$self`: the patterns of the grammar the rule is in.
    This,
    /// `$base`: the patterns of the grammar the document uses.
    Base,
    /// `#name`: a rule of the repository of the grammar.
    Repository(String),
    /// `scope.name` or `scope.name#rule`: another grammar.
    External { scope: String, name: Option<String> },
}

pub(super) enum RuleRef {
    Rule(usize),
    Include(Include),
}

/// What a capture group of a match is scoped as and tokenized with.
#[derive(Default)]
pub(super) struct Capture {
    pub name: Option<String>,
    /// A container rule of the capture's own `patterns`.
    pub container: Option<usize>,
}

pub(super) type CaptureMap = BTreeMap<usize, Capture>;

pub(super) enum Kind {
    Container(Vec<RuleRef>),
    Match {
        pattern: Pattern,
        captures: CaptureMap,
    },
    BeginEnd {
        begin: Pattern,
        end: EndPattern,
        begin_captures: CaptureMap,
        end_captures: CaptureMap,
        patterns: Vec<RuleRef>,
        apply_end_last: bool,
    },
    BeginWhile {
        begin: Pattern,
        condition: Pattern,
        begin_captures: CaptureMap,
        while_captures: CaptureMap,
        patterns: Vec<RuleRef>,
    },
}

pub(super) struct Rule {
    pub grammar: usize,
    pub name: Option<String>,
    pub content_name: Option<String>,
    pub kind: Kind,
}

pub(super) struct GrammarData {
    pub scope_name: String,
    pub root: Vec<RuleRef>,
    pub repository: HashMap<String, usize>,
}

/// Finds the JSON of a grammar by its scope name.
type GrammarLoader = Box<dyn Fn(&str) -> Option<Value>>;

/// The grammars loaded so far, with all their rules.
pub struct Registry {
    pub(super) rules: Vec<Rule>,
    pub(super) grammars: Vec<GrammarData>,
    by_scope: HashMap<String, usize>,
    failed: HashSet<String>,
    loader: GrammarLoader,
    /// Compiled end patterns by their source: backreferences make many of them per rule.
    end_patterns: RefCell<HashMap<String, Rc<Pattern>>>,
}

/// Collects what parsing a grammar refers to in other grammars.
#[derive(Default)]
struct Parser {
    externals: Vec<String>,
}

impl Registry {
    /// A registry that finds grammars it is asked to include by their scope name with `loader`.
    pub fn new(loader: impl Fn(&str) -> Option<Value> + 'static) -> Self {
        Self {
            rules: Vec::new(),
            grammars: Vec::new(),
            by_scope: HashMap::new(),
            failed: HashSet::new(),
            loader: Box::new(loader),
            end_patterns: RefCell::new(HashMap::new()),
        }
    }

    /// The index of the loaded grammar of `scope`.
    pub(super) fn index_of(&self, scope: &str) -> Option<usize> {
        self.by_scope.get(scope).copied()
    }

    /// The compiled end pattern with this source.
    pub(super) fn end_pattern(&self, source: &str) -> Rc<Pattern> {
        let mut cache = self.end_patterns.borrow_mut();
        if let Some(pattern) = cache.get(source) {
            return pattern.clone();
        }
        if cache.len() > 4_096 {
            cache.clear();
        }
        let pattern = Rc::new(Pattern::new(source));
        cache.insert(source.to_owned(), pattern.clone());
        pattern
    }

    /// The pattern that starts a `match` or `begin` rule.
    pub(super) fn begin_pattern(&self, rule: usize) -> &Pattern {
        match &self.rules[rule].kind {
            Kind::Match { pattern, .. } => pattern,
            Kind::BeginEnd { begin, .. } | Kind::BeginWhile { begin, .. } => begin,
            Kind::Container(_) => unreachable!("containers are expanded into their rules"),
        }
    }

    /// The index of the grammar of `scope`, loading it (and what it includes) if needed.
    pub fn grammar(&mut self, scope: &str) -> Option<usize> {
        if let Some(index) = self.by_scope.get(scope) {
            return Some(*index);
        }
        if self.failed.contains(scope) {
            return None;
        }
        let loaded = (self.loader)(scope).and_then(|value| self.add(&value).ok());
        if loaded.is_none() {
            self.failed.insert(scope.to_owned());
        }
        loaded
    }

    /// Adds a grammar given as its JSON value. Returns its index.
    pub fn add(&mut self, value: &Value) -> Result<usize, String> {
        let scope = value["scopeName"]
            .as_str()
            .filter(|scope| !scope.is_empty())
            .ok_or("A grammar has no scopeName")?
            .to_owned();
        if let Some(index) = self.by_scope.get(&scope) {
            return Ok(*index);
        }
        let index = self.grammars.len();
        // Registered before its includes load, so grammars that include each other terminate.
        self.grammars.push(GrammarData {
            scope_name: scope.clone(),
            root: Vec::new(),
            repository: HashMap::new(),
        });
        self.by_scope.insert(scope, index);
        let mut parser = Parser::default();
        let root = self.parse_patterns(&mut parser, &value["patterns"], index);
        let mut repository = HashMap::new();
        if let Some(entries) = value["repository"].as_object() {
            for (name, rule) in entries {
                let id = self.parse_rule(&mut parser, rule, index);
                repository.insert(name.clone(), id);
            }
        }
        self.grammars[index].root = root;
        self.grammars[index].repository = repository;
        for scope in parser.externals {
            self.grammar(&scope);
        }
        Ok(index)
    }

    /// Adds a grammar from the text of a `.tmLanguage.json` or XML plist file.
    pub fn add_text(&mut self, text: &str) -> Result<usize, String> {
        let value = parse_grammar_text(text)?;
        self.add(&value)
    }

    pub fn scope_name(&self, grammar: usize) -> &str {
        &self.grammars[grammar].scope_name
    }

    fn parse_patterns(
        &mut self,
        parser: &mut Parser,
        value: &Value,
        grammar: usize,
    ) -> Vec<RuleRef> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter(|entry| entry["disabled"].as_i64().unwrap_or(0) == 0)
            .map(|entry| {
                if let Some(include) = entry["include"].as_str() {
                    return RuleRef::Include(parse_include(parser, include));
                }
                RuleRef::Rule(self.parse_rule(parser, entry, grammar))
            })
            .collect()
    }

    fn parse_captures(&mut self, parser: &mut Parser, value: &Value, grammar: usize) -> CaptureMap {
        let mut captures = CaptureMap::new();
        for (key, entry) in value.as_object().into_iter().flatten() {
            let Ok(index) = key.parse::<usize>() else {
                continue;
            };
            let container = entry["patterns"].is_array().then(|| {
                let patterns = self.parse_patterns(parser, &entry["patterns"], grammar);
                self.push(Rule {
                    grammar,
                    name: None,
                    content_name: None,
                    kind: Kind::Container(patterns),
                })
            });
            captures.insert(
                index,
                Capture {
                    name: entry["name"].as_str().map(str::to_owned),
                    container,
                },
            );
        }
        captures
    }

    fn push(&mut self, rule: Rule) -> usize {
        self.rules.push(rule);
        self.rules.len() - 1
    }

    fn parse_rule(&mut self, parser: &mut Parser, value: &Value, grammar: usize) -> usize {
        let name = value["name"].as_str().map(str::to_owned);
        let content_name = value["contentName"].as_str().map(str::to_owned);
        let both = value.get("captures");
        let captures_of = |this: &mut Self, parser: &mut Parser, key: &str| {
            let specific = value.get(key).or(both).cloned().unwrap_or(Value::Null);
            this.parse_captures(parser, &specific, grammar)
        };
        let kind = if let Some(source) = value["match"].as_str() {
            Kind::Match {
                pattern: Pattern::new(source),
                captures: captures_of(self, parser, "captures"),
            }
        } else if let Some(begin) = value["begin"].as_str() {
            let patterns = self.parse_patterns(parser, &value["patterns"], grammar);
            if let Some(end) = value["end"].as_str() {
                Kind::BeginEnd {
                    begin: Pattern::new(begin),
                    end: EndPattern::new(end),
                    begin_captures: captures_of(self, parser, "beginCaptures"),
                    end_captures: captures_of(self, parser, "endCaptures"),
                    patterns,
                    apply_end_last: value["applyEndPatternLast"].as_i64().unwrap_or(0) != 0
                        || value["applyEndPatternLast"].as_bool().unwrap_or(false),
                }
            } else if let Some(condition) = value["while"].as_str() {
                Kind::BeginWhile {
                    begin: Pattern::new(begin),
                    condition: Pattern::new(condition),
                    begin_captures: captures_of(self, parser, "beginCaptures"),
                    while_captures: captures_of(self, parser, "whileCaptures"),
                    patterns,
                }
            } else {
                Kind::Container(patterns)
            }
        } else if let Some(include) = value["include"].as_str() {
            Kind::Container(vec![RuleRef::Include(parse_include(parser, include))])
        } else {
            Kind::Container(self.parse_patterns(parser, &value["patterns"], grammar))
        };
        self.push(Rule {
            grammar,
            name,
            content_name,
            kind,
        })
    }
}

fn parse_include(parser: &mut Parser, include: &str) -> Include {
    match include {
        "$self" => Include::This,
        "$base" => Include::Base,
        _ => match include.strip_prefix('#') {
            Some(name) => Include::Repository(name.to_owned()),
            None => {
                let (scope, name) = match include.split_once('#') {
                    Some((scope, name)) => (scope, Some(name.to_owned())),
                    None => (include, None),
                };
                parser.externals.push(scope.to_owned());
                Include::External {
                    scope: scope.to_owned(),
                    name,
                }
            }
        },
    }
}

/// Reads the text of a grammar file: JSON, or an XML property list.
pub fn parse_grammar_text(text: &str) -> Result<Value, String> {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    if trimmed.starts_with('<') {
        plist::from_bytes::<Value>(trimmed.as_bytes()).map_err(|error| error.to_string())
    } else {
        serde_json::from_str(trimmed).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn oniguruma_only_syntax_is_translated() {
        let (translated, anchored) = translate(r"\G\h+[\h_]\H");
        assert!(anchored);
        assert!(translated.starts_with("(?m)"));
        assert!(translated.contains("[0-9a-fA-F]+"));
        assert!(translated.contains("[0-9a-fA-F_]"));
        assert!(translated.contains("[^0-9a-fA-F]"));
        assert!(!translated.contains("\\G"));
        assert!(Pattern::new(r"[]a]").compiles(), "a leading ] is a member");
        assert!(Pattern::new(r"[^]a]+").compiles());
        assert!(!Pattern::new(r"(unclosed").compiles());
        assert!(Pattern::new(r"(?<name>x)\k<name>").compiles());
    }

    #[test]
    fn patterns_find_groups_and_honor_the_anchor() {
        let pattern = Pattern::new(r"(a)(b)?");
        let groups = pattern.find("xab", 0, None).unwrap();
        assert_eq!(groups[0], Some((1, 3)));
        assert_eq!(groups[2], Some((2, 3)));
        let anchored = Pattern::new(r"\Gab");
        assert!(
            anchored.find("xab", 0, Some(0)).is_none(),
            "ab is at 1, not at the anchor"
        );
        assert!(anchored.find("xab", 0, Some(1)).is_some());
        assert!(
            anchored.find("xab", 0, None).is_none(),
            "no anchor, no match"
        );
        assert!(Pattern::new("(bad").find("x", 0, None).is_none());
    }

    #[test]
    fn end_patterns_substitute_the_begin_groups() {
        let end = EndPattern::new(r"^\1$|\\\1x");
        assert!(end.has_backrefs);
        let line = "<<EOF.*";
        let begin = Pattern::new(r"<<(\w+)\.\*").find(line, 0, None).unwrap();
        assert_eq!(end.resolve(line, &begin), r"^EOF$|\\EOFx");
        let tricky = "<<a.b";
        let groups = vec![Some((0, 5)), Some((2, 5))];
        assert_eq!(
            end.resolve(tricky, &groups),
            r"^a\.b$|\\a\.bx",
            "group text is escaped"
        );
        assert!(!EndPattern::new("x").has_backrefs);
    }

    #[test]
    fn grammars_load_from_json_and_plist_and_link_their_includes() {
        let mut registry = Registry::new(|scope| {
            (scope == "source.inner").then(|| {
                json!({"scopeName": "source.inner", "patterns": [{"match": "x", "name": "keyword.x"}]})
            })
        });
        let outer = registry
            .add(&json!({
                "scopeName": "source.outer",
                "patterns": [{"include": "#a"}, {"include": "source.inner"}, {"include": "source.missing#r"}],
                "repository": {"a": {"match": "a", "name": "constant.a"}}
            }))
            .unwrap();
        assert_eq!(registry.scope_name(outer), "source.outer");
        assert!(
            registry.grammar("source.inner").is_some(),
            "loaded for the include"
        );
        assert!(registry.grammar("source.missing").is_none());
        assert_eq!(
            registry.add(&json!({"scopeName": "source.outer"})).unwrap(),
            outer,
            "added once"
        );
        assert!(registry.add(&json!({"patterns": []})).is_err());

        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
 <key>scopeName</key><string>source.plist</string>
 <key>patterns</key><array><dict><key>match</key><string>\d+</string><key>name</key><string>constant.numeric.plist</string></dict></array>
</dict></plist>"#;
        let mut registry = Registry::new(|_| None);
        let index = registry.add_text(plist).unwrap();
        assert_eq!(registry.scope_name(index), "source.plist");
        assert!(registry.add_text("{not json").is_err());
        assert!(registry
            .add_text("\u{feff}{\"scopeName\": \"source.bom\", \"patterns\": []}")
            .is_ok());
    }
}
