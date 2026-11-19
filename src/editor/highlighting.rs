use crate::editor::language::TsGrammar;
use crate::editor::{Buffer, Language};
use crate::themes::SyntaxRole;
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Tree};

/// Byte range within a line, tagged with a semantic syntax role.
pub type Span = (usize, usize, SyntaxRole);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Syntax context at the caret, used to suppress completion inside strings or
/// comments and to detect member-access expressions via Tree-sitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContext {
    Code,
    Comment,
    String,
    MemberAccess,
}

#[derive(Debug, Clone)]
pub struct SyntaxDiagnostic {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

/// Cached syntax information for one buffer. Recomputed only when the buffer
/// revision or language changes.
pub struct Highlighter {
    parser: Option<Parser>,
    tree: Option<Tree>,
    language: Language,
    revision: u64,
    spans: Vec<Vec<Span>>,
    block_comment: bool,
    block_string: bool,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            parser: None,
            tree: None,
            language: Language::PlainText,
            revision: u64::MAX,
            spans: Vec::new(),
            block_comment: false,
            block_string: false,
        }
    }
}

impl Highlighter {
    pub fn new() -> Self { Self::default() }

    /// Return highlighted spans per line. The full buffer text is materialized
    /// only when the buffer actually changed, keeping idle frames cheap.
    pub fn spans(&mut self, buffer: &Buffer, revision: u64) -> &[Vec<Span>] {
        if self.language != buffer.language || self.revision != revision {
            let text = buffer.text();
            self.recompute(buffer.language, &text, revision);
        }
        &self.spans
    }

    pub fn invalidate(&mut self) {
        self.revision = u64::MAX;
    }

    /// Determine the Tree-sitter syntax context covering a byte offset.
    pub fn context_at_byte(&self, byte: usize) -> CompletionContext {
        let Some(tree) = &self.tree else { return CompletionContext::Code; };
        let root = tree.root_node();
        let node = deepest_node_at(root, byte);
        let mut context = CompletionContext::Code;
        let kind = node.kind();
        if kind.contains("comment") { context = CompletionContext::Comment; }
        else if kind.contains("string") || kind.contains("char_literal") { context = CompletionContext::String; }
        else if matches!(kind, "field_expression" | "member_expression" | "attribute" | "property_access") {
            context = CompletionContext::MemberAccess;
        }
        context
    }

    /// Syntax errors reported by tree-sitter, used by the Problems panel.
    pub fn diagnostics(&self) -> Vec<SyntaxDiagnostic> {
        let Some(tree) = &self.tree else { return Vec::new(); };
        let mut out = Vec::new();
        fn walk(node: Node<'_>, out: &mut Vec<SyntaxDiagnostic>) {
            if node.is_error() {
                let p = node.start_position();
                out.push(SyntaxDiagnostic {
                    line: p.row + 1,
                    col: p.column + 1,
                    message: "Unexpected syntax error".into(),
                    severity: DiagnosticSeverity::Error,
                });
            } else if node.is_missing() {
                let p = node.start_position();
                out.push(SyntaxDiagnostic {
                    line: p.row + 1,
                    col: p.column + 1,
                    message: format!("Missing {}", node.kind()),
                    severity: DiagnosticSeverity::Warning,
                });
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk(child, out);
            }
        }
        walk(tree.root_node(), &mut out);
        out.truncate(200);
        out
    }

    fn recompute(&mut self, language: Language, text: &str, revision: u64) {
        if self.language != language {
            self.tree = None;
        }
        self.language = language;
        self.revision = revision;
        self.spans.clear();

        let line_count = text.split('\n').count();
        self.spans.resize(line_count, Vec::new());

        if let Some(grammar) = language.ts_grammar() {
            if let Some(mut parser) = self.build_parser(grammar) {
                let old_tree = self.tree.take();
                if let Some(tree) = parser.parse(text, old_tree.as_ref()) {
                    let mut ranges = Vec::new();
                    collect_nodes(tree.root_node(), language, &mut ranges);
                    distribute(text, &mut self.spans, ranges);
                    self.tree = Some(tree);
                    self.parser = Some(parser);
                    return;
                }
                self.parser = Some(parser);
            }
        }
        self.fallback(text, language);
    }

    fn build_parser(&self, grammar: TsGrammar) -> Option<Parser> {
        let mut parser = Parser::new();
        let ok = match grammar {
            TsGrammar::Rust => parser.set_language(&tree_sitter_rust::language()),
            TsGrammar::JavaScript => parser.set_language(&tree_sitter_javascript::language()),
            TsGrammar::Python => parser.set_language(&tree_sitter_python::language()),
        };
        ok.ok()?;
        Some(parser)
    }

    /// Regex-free tokenizer used for languages without a tree-sitter grammar
    /// (and as a safety net if a grammar fails to load). It understands
    /// language-specific tokens such as JSON keys, TOML tables, YAML keys,
    /// Markdown headings, C preprocessor directives and Rust macros.
    fn fallback(&mut self, text: &str, language: Language) {
        self.block_comment = false;
        self.block_string = false;
        let keywords = language.keywords();
        let comment_prefix = language.comment_prefix();
        for (line_idx, line) in text.split('\n').enumerate() {
            let mut spans = Vec::new();
            let chars: Vec<char> = line.chars().collect();
            let mut byte_at = Vec::with_capacity(chars.len() + 1);
            byte_at.push(0usize);
            for c in &chars { byte_at.push(byte_at.last().unwrap() + c.len_utf8()); }
            let line_bytes = *byte_at.last().unwrap();
            let trimmed = line.trim_start();
            let leading = line.len() - trimmed.len();

            // ---- Line-level structural tokens ---------------------------
            match language {
                Language::Markdown => {
                    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
                    if hashes > 0 && hashes <= 6 && trimmed.chars().nth(hashes) == Some(' ') {
                        spans.push((leading, leading + hashes, SyntaxRole::Keyword));
                        spans.push((leading + hashes + 1, line_bytes, SyntaxRole::Type));
                        self.spans.get_mut(line_idx).map(|s| *s = spans);
                        continue;
                    }
                    if trimmed.starts_with("```") {
                        spans.push((leading, line_bytes, SyntaxRole::String));
                        self.spans.get_mut(line_idx).map(|s| *s = spans);
                        continue;
                    }
                }
                Language::C | Language::Cpp if trimmed.starts_with('#') => {
                    let word_end = trimmed.chars().skip(1)
                        .take_while(|c| c.is_ascii_alphabetic()).count() + 1;
                    spans.push((leading, leading + word_end, SyntaxRole::Keyword));
                    let rest: String = trimmed.chars().skip(word_end).collect();
                    if rest.trim_start().starts_with('<') {
                        if let Some(end) = rest.find('>') {
                            spans.push((leading + word_end + rest.find('<').unwrap(),
                                leading + word_end + end + 1, SyntaxRole::String));
                        }
                    } else if rest.trim_start().starts_with('"') {
                        let start = word_end + rest.find('"').unwrap();
                        let end = rest.rfind('"').unwrap() + 1;
                        spans.push((leading + start, leading + end, SyntaxRole::String));
                    }
                    self.spans.get_mut(line_idx).map(|s| *s = spans);
                    continue;
                }
                Language::Toml => {
                    if trimmed.starts_with('[') {
                        spans.push((leading, line_bytes, SyntaxRole::Type));
                        self.spans.get_mut(line_idx).map(|s| *s = spans);
                        continue;
                    }
                    if let Some(eq) = trimmed.find('=') {
                        let key = trimmed[..eq].trim();
                        if !key.is_empty() {
                            spans.push((leading, leading + key.len(), SyntaxRole::Property));
                        }
                    }
                }
                Language::Yaml => {
                    if let Some(colon) = trimmed.find(':') {
                        let key = trimmed[..colon].trim();
                        if !key.is_empty() && !key.starts_with('-') {
                            spans.push((leading, leading + key.len(), SyntaxRole::Property));
                        }
                    }
                }
                _ => {}
            }

            // ---- Character-level tokenizer -------------------------------
            let mut i = 0usize;
            while i < chars.len() {
                let c = chars[i];
                let start_byte = byte_at[i];

                if self.block_comment {
                    if i + 1 < chars.len() && c == '*' && chars[i + 1] == '/' {
                        self.block_comment = false;
                        i += 2;
                        spans.push((start_byte, byte_at[i], SyntaxRole::Comment));
                    } else { i += 1; }
                    continue;
                }
                if self.block_string {
                    if c == '"' || c == '\'' { self.block_string = false; }
                    i += 1;
                    continue;
                }

                // Line comments.
                if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' && comment_prefix == "//" {
                    spans.push((start_byte, line_bytes, SyntaxRole::Comment));
                    break;
                }
                if c == '#' && comment_prefix == "#" {
                    spans.push((start_byte, line_bytes, SyntaxRole::Comment));
                    break;
                }
                if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                    self.block_comment = true;
                    i += 2;
                    continue;
                }
                // HTML/XML tags and attributes.
                if matches!(language, Language::Html) && c == '<' {
                    let mut end = i + 1;
                    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '/' || chars[end] == '!' || chars[end] == '-') { end += 1; }
                    if end > i + 1 {
                        spans.push((start_byte, byte_at[end], SyntaxRole::Tag));
                        i = end;
                        continue;
                    }
                }
                // Strings, with escape highlighting.
                if c == '"' || c == '\'' || c == '`' {
                    let quote = c;
                    let triple = i + 2 < chars.len() && chars[i + 1] == quote && chars[i + 2] == quote;
                    i += if triple { 3 } else { 1 };
                    let begin = start_byte;
                    let mut escapes = Vec::new();
                    let mut closed = false;
                    while i < chars.len() {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            escapes.push((byte_at[i], byte_at[i + 2]));
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            if triple && i + 2 < chars.len() && chars[i + 1] == quote && chars[i + 2] == quote { i += 3; }
                            else { i += 1; }
                            closed = true;
                            break;
                        }
                        i += 1;
                    }
                    if triple && !closed { self.block_string = true; }
                    let end_byte = byte_at[i.min(chars.len())];
                    // JSON object keys are strings followed by a colon.
                    let is_json_key = language == Language::Json
                        && chars[i..].iter().position(|c| !c.is_whitespace())
                            .map(|off| chars.get(i + off) == Some(&':'))
                            .unwrap_or(false);
                    spans.push((begin, end_byte,
                        if is_json_key { SyntaxRole::Property } else { SyntaxRole::String }));
                    for (s, e) in escapes { spans.push((s, e, SyntaxRole::StringEscape)); }
                    continue;
                }
                // Numbers (including hex/binary suffixes and digit separators).
                if c.is_ascii_digit() {
                    let mut end = i + 1;
                    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '.' || chars[end] == '_') { end += 1; }
                    spans.push((start_byte, byte_at[end], SyntaxRole::Number));
                    i = end;
                    continue;
                }
                // Rust lifetimes and Python decorators.
                if (language == Language::Rust && c == '\'' && i + 1 < chars.len() && chars[i + 1].is_alphabetic())
                    || (matches!(language, Language::Python) && c == '@') {
                    let mut end = i + 1;
                    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') { end += 1; }
                    let role = if language == Language::Rust { SyntaxRole::TypeParameter } else { SyntaxRole::Attribute };
                    spans.push((start_byte, byte_at[end], role));
                    i = end;
                    continue;
                }
                // Identifiers: keywords, macros, functions, JSON keys.
                if c.is_alphabetic() || c == '_' {
                    let mut end = i + 1;
                    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') { end += 1; }
                    let word: String = chars[i..end].iter().collect();
                    let role = if keywords.contains(&word.as_str()) {
                        control_role(&word)
                    } else if language == Language::Json && end < chars.len() && chars[end] == ':' {
                        SyntaxRole::Property
                    } else if end < chars.len() && chars[end] == '!' {
                        SyntaxRole::Macro
                    } else if end < chars.len() && chars[end] == '(' {
                        SyntaxRole::Function
                    } else {
                        SyntaxRole::Plain
                    };
                    if role != SyntaxRole::Plain { spans.push((start_byte, byte_at[end], role)); }
                    i = end;
                    continue;
                }
                // Operators.
                if "=+-*/<>!&|^%?:~".contains(c) {
                    spans.push((start_byte, byte_at[i + 1], SyntaxRole::Operator));
                }
                i += 1;
            }
            if let Some(line_spans) = self.spans.get_mut(line_idx) {
                *line_spans = spans;
            }
        }
    }
}

fn deepest_node_at<'a>(node: tree_sitter::Node<'a>, byte: usize) -> tree_sitter::Node<'a> {
    let mut cursor = node.walk();
    let mut best = node;
    for child in node.children(&mut cursor) {
        let start = child.start_byte();
        let end = child.end_byte();
        if byte >= start && byte < end {
            best = deepest_node_at(child, byte);
            break;
        }
    }
    best
}

fn control_role(word: &str) -> SyntaxRole {
    const CONTROL: &[&str] = &[
        "if", "else", "elif", "match", "switch", "case", "for", "while", "loop", "do",
        "return", "break", "continue", "try", "catch", "finally", "throw", "raise",
        "await", "async", "yield", "with", "in", "goto", "select", "defer",
    ];
    if CONTROL.contains(&word) { SyntaxRole::ControlKeyword } else { SyntaxRole::Keyword }
}

fn keyword_role(kind: &str) -> Option<SyntaxRole> {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
        "trait", "true", "type", "unsafe", "use", "where", "while", "yield", "function",
        "var", "class", "new", "delete", "typeof", "instanceof", "this", "import", "export",
        "from", "default", "extends", "switch", "case", "try", "catch", "finally", "throw",
        "def", "lambda", "pass", "raise", "with", "global", "nonlocal", "del", "assert",
        "and", "or", "not", "is", "True", "False", "None", "elif", "except", "nil",
        "int", "char", "float", "double", "void", "long", "short", "unsigned", "signed",
        "sizeof", "template", "namespace", "virtual", "inline", "typedef", "union", "goto",
        "package", "func", "go", "defer", "chan", "map", "range", "select", "fallthrough",
        "interface", "final", "abstract", "native", "synchronized", "transient", "volatile",
    ];
    if KEYWORDS.contains(&kind) { Some(control_role(kind)) } else { None }
}

fn kind_role(kind: &str) -> Option<SyntaxRole> {
    match kind {
        "line_comment" | "block_comment" | "comment" | "comment_block" => Some(SyntaxRole::Comment),
        "string" | "string_literal" | "char_literal" | "template_string" | "string_content"
        | "template_string_type" => Some(SyntaxRole::String),
        "escape_sequence" | "string_escape" => Some(SyntaxRole::StringEscape),
        "regex" | "regexp" => Some(SyntaxRole::Regexp),
        "number" | "integer" | "integer_literal" | "float_literal" | "numeric_literal" => Some(SyntaxRole::Number),
        "primitive_type" | "type_identifier" | "type" | "class" | "struct_item" | "enum_item"
        | "interface_declaration" | "class_declaration" | "struct_specifier" | "union_specifier" => Some(SyntaxRole::Type),
        "type_parameter" | "type_parameters" | "lifetime" => Some(SyntaxRole::TypeParameter),
        "enum_variant" | "enumerator" | "enum_assignment" => Some(SyntaxRole::EnumMember),
        "property_identifier" | "field_identifier" | "shorthand_property_identifier"
        | "pair" | "property" | "field" => Some(SyntaxRole::Property),
        "constant" | "true" | "false" | "null" | "boolean" => Some(SyntaxRole::Constant),
        "operator" | "binary_operator" | "assignment_operator" | "unary_operator"
        | "comparison_operator" | "logical_operator" | "arrow" => Some(SyntaxRole::Operator),
        "attribute_item" | "attribute" | "decorator" | "meta_item" => Some(SyntaxRole::Attribute),
        "macro_invocation" | "macro_definition" => Some(SyntaxRole::Macro),
        "tag_name" | "doctype" => Some(SyntaxRole::Tag),
        "attribute_name" | "attribute_value" => Some(SyntaxRole::TagAttribute),
        k => keyword_role(k),
    }
}

/// Walk the syntax tree and emit byte ranges. Field-specific children are
/// tagged precisely (function names, call targets, variable names, etc.).
fn collect_nodes(node: Node<'_>, _language: Language, out: &mut Vec<(usize, usize, SyntaxRole)>) {
    let kind = node.kind();

    match kind {
        "function_item" | "function_signature_item" | "function_declaration" | "function_definition" | "method_definition" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push((name.start_byte(), name.end_byte(), SyntaxRole::Function));
            }
        }
        "call_expression" => {
            if let Some(fun) = node.child_by_field_name("function") {
                out.push((fun.start_byte(), fun.end_byte(), SyntaxRole::Function));
            }
        }
        "variable_declarator" | "let_declaration" | "assignment" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push((name.start_byte(), name.end_byte(), SyntaxRole::Variable));
            }
        }
        "property_definition" | "field_declaration" | "public_field_definition" | "pair" => {
            if let Some(name) = node.child_by_field_name("key").or_else(|| node.child_by_field_name("name")) {
                out.push((name.start_byte(), name.end_byte(), SyntaxRole::Property));
            }
        }
        "enum_variant" | "enumerator" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push((name.start_byte(), name.end_byte(), SyntaxRole::EnumMember));
            }
        }
        "macro_invocation" | "macro_definition" => {
            if let Some(name) = node.child_by_field_name("macro") {
                out.push((name.start_byte(), name.end_byte(), SyntaxRole::Macro));
            }
        }
        "type_parameter" | "type_identifier" => {
            out.push((node.start_byte(), node.end_byte(), SyntaxRole::TypeParameter));
        }
        _ => {}
    }

    if let Some(role) = kind_role(kind) {
        out.push((node.start_byte(), node.end_byte(), role));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, _language, out);
    }
}

/// Convert global byte ranges into per-line clipped ranges. Inner/more
/// specific roles override earlier ones when ranges overlap.
fn distribute(text: &str, spans: &mut [Vec<Span>], ranges: Vec<(usize, usize, SyntaxRole)>) {
    let mut line_starts = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' { line_starts.push(i + 1); }
    }
    let line_ends = |line: usize| -> usize {
        if line + 1 < line_starts.len() {
            line_starts[line + 1].saturating_sub(1)
        } else {
            text.len()
        }
    };

    for (start, end, role) in ranges {
        if start >= end { continue; }
        let first = match line_starts.binary_search(&start) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let mut line = first;
        while line < spans.len() && line_starts[line] < end {
            let ls = line_starts[line];
            let le = line_ends(line);
            let a = start.max(ls);
            let b = end.min(le);
            if a < b {
                spans[line].push((a - ls, b - ls, role));
            }
            line += 1;
        }
    }

    // Convert each line's byte spans into non-overlapping char-independent
    // ranges by sorting; later (inner) roles overwrite earlier ones.
    for line_spans in spans.iter_mut() {
        line_spans.sort_by_key(|s| (s.0, s.1));
    }
}

/// Build ratatui spans for a single line given byte-based highlight ranges.
pub fn spans_for_line(text: &str, ranges: &[Span], theme: &crate::themes::Theme) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::style::Style;
    use ratatui::text::Span;
    let mut roles: Vec<SyntaxRole> = vec![SyntaxRole::Plain; text.len()];
    for &(start, end, role) in ranges {
        for b in start.min(text.len())..end.min(text.len()) {
            roles[b] = role;
        }
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_role = SyntaxRole::Plain;
    let mut char_byte = 0usize;
    for c in text.chars() {
        let role = roles.get(char_byte).copied().unwrap_or(SyntaxRole::Plain);
        if current.is_empty() {
            current_role = role;
        } else if role != current_role {
            out.push(Span::styled(std::mem::take(&mut current), theme.style(current_role)));
            current_role = role;
        }
        current.push(c);
        char_byte += c.len_utf8();
    }
    if !current.is_empty() {
        out.push(Span::styled(current, theme.style(current_role)));
    }
    if out.is_empty() {
        out.push(Span::styled(String::new(), Style::default()));
    }
    out
}

/// Cache keyed by buffer id so each open tab keeps its parse tree.
pub struct HighlightRegistry {
    cache: HashMap<usize, Highlighter>,
}

impl HighlightRegistry {
    pub fn new() -> Self { Self { cache: HashMap::new() } }

    pub fn for_buffer(&mut self, id: usize) -> &mut Highlighter {
        self.cache.entry(id).or_insert_with(Highlighter::new)
    }

    pub fn remove(&mut self, id: usize) {
        self.cache.remove(&id);
    }
}
