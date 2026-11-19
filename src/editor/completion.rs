//! Context-aware code completion. Suggestions are derived from the live
//! buffer, the workspace and the active language grammar. The engine is
//! intentionally self-contained so an LSP-backed provider can be added later
//! behind the same `CompletionItem` interface.

use super::buffer::Buffer;
use super::highlighting::CompletionContext;
use super::Language;
use crate::services::Config;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Variable,
    Function,
    Method,
    Class,
    Field,
    Constant,
    Module,
}

impl CompletionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "K",
            CompletionKind::Variable => "v",
            CompletionKind::Function => "ƒ",
            CompletionKind::Method => "m",
            CompletionKind::Class => "C",
            CompletionKind::Field => "f",
            CompletionKind::Constant => "A",
            CompletionKind::Module => "M",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "keyword",
            CompletionKind::Variable => "variable",
            CompletionKind::Function => "function",
            CompletionKind::Method => "method",
            CompletionKind::Class => "class",
            CompletionKind::Field => "field",
            CompletionKind::Constant => "constant",
            CompletionKind::Module => "module",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub insert_text: String,
    pub score: i32,
}

#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub visible: bool,
    pub prefix: String,
    pub replace_start: usize,
    pub replace_end: usize,
    pub line: usize,
}

impl CompletionState {
    pub fn clear(&mut self) { *self = Self::default(); }

    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() { return; }
        let len = self.items.len() as isize;
        self.selected = ((self.selected as isize + delta) % len + len) as usize % len as usize;
    }

    pub fn accept(&self) -> Option<&CompletionItem> { self.items.get(self.selected) }
}

#[derive(Debug, Clone)]
struct Candidate {
    name: String,
    kind: CompletionKind,
    detail: String,
    local: bool,
    count: usize,
}

/// Completes identifiers from the current file + workspace, language keywords
/// and (best-effort) type-aware members after `.`.
pub struct CompletionEngine {
    workspace: HashMap<String, Candidate>,
    workspace_key: Option<(PathBuf, usize)>,
}

impl Default for CompletionEngine {
    fn default() -> Self { Self { workspace: HashMap::new(), workspace_key: None } }
}

impl CompletionEngine {
    pub fn new() -> Self { Self::default() }

    pub fn compute(
        &mut self,
        buffer: &Buffer,
        cursor: usize,
        workspace_root: Option<&Path>,
        config: &Config,
        context: CompletionContext,
    ) -> Option<CompletionState> {
        if !config.autocomplete { return None; }
        if matches!(context, CompletionContext::Comment | CompletionContext::String) { return None; }
        let text = buffer.text();
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() { return None; }
        let cursor = cursor.min(chars.len());

        // 1. Current token prefix.
        let mut start = cursor;
        while start > 0 {
            let c = chars[start - 1];
            if c.is_alphanumeric() || c == '_' { start -= 1; } else { break; }
        }
        let prefix: String = chars[start..cursor].iter().collect();
        let member_access = (start > 0 && chars[start - 1] == '.')
            || context == CompletionContext::MemberAccess;

        // Don't pop up on a single character unless it follows '.' or explicit
        // request; this keeps ordinary typing uninterrupted.
        if prefix.is_empty() && !member_access { return None; }
        if prefix.len() < 2 && !member_access { return None; }

        let line = buffer.char_to_line(start);
        let mut candidates: Vec<Candidate> = Vec::new();

        if member_access {
            self.member_candidates(&chars, start, buffer.language, &mut candidates);
        }

        self.buffer_candidates(buffer, &mut candidates);
        if config.autocomplete_workspace {
            if let Some(root) = workspace_root {
                let current = buffer.path().map(|p| p.to_path_buf());
                self.workspace_candidates(root, buffer.language, current.as_ref(), &mut candidates);
            }
        }
        for keyword in buffer.language.keywords() {
            if !keyword.starts_with(|c: char| c.is_alphanumeric() || c == '_') { continue; }
            candidates.push(Candidate {
                name: (*keyword).to_string(),
                kind: CompletionKind::Keyword,
                detail: "keyword".into(),
                local: false,
                count: 0,
            });
        }

        // 2. Rank and filter.
        let needle = prefix.to_lowercase();
        let mut items: Vec<CompletionItem> = candidates
            .into_iter()
            .filter(|c| {
                let lower = c.name.to_lowercase();
                needle.is_empty() || lower.starts_with(&needle) || (!member_access && lower.contains(&needle))
            })
            .map(|c| {
                let lower = c.name.to_lowercase();
                let mut score = if lower.starts_with(&needle) { 100 } else if lower.contains(&needle) { 40 } else { 0 };
                if c.local { score += 30; }
                if member_access && matches!(c.kind, CompletionKind::Method | CompletionKind::Field) { score += 25; }
                if c.name == needle { score += 20; }
                score += c.count.min(10) as i32;
                CompletionItem {
                    label: c.name.clone(),
                    kind: c.kind,
                    detail: c.detail,
                    insert_text: c.name,
                    score,
                }
            })
            .collect();
        items.sort_by(|a, b| b.score.cmp(&a.score).then(a.label.cmp(&b.label)));
        let max = config.autocomplete_max_items.max(1);
        items.truncate(max);
        if items.is_empty() { return None; }

        Some(CompletionState {
            items,
            selected: 0,
            visible: true,
            prefix,
            replace_start: start,
            replace_end: cursor,
            line,
        })
    }

    /// Local identifiers from the current buffer, classified by context.
    fn buffer_candidates(&self, buffer: &Buffer, out: &mut Vec<Candidate>) {
        let text = buffer.text();
        let lines: Vec<&str> = text.lines().collect();
        let mut class_names: Vec<String> = Vec::new();
        for line in &lines {
            for pattern in ["class ", "struct ", "enum ", "interface ", "trait "] {
                if let Some(pos) = line.find(pattern) {
                    let name: String = line[pos + pattern.len()..].chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    if !name.is_empty() { class_names.push(name); }
                }
            }
        }

        for line in &lines {
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0usize;
            while i < chars.len() {
                if chars[i].is_alphabetic() || chars[i] == '_' {
                    let begin = i;
                    i += 1;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
                    let name: String = chars[begin..i].iter().collect();
                    if name.len() < 2 || class_names.contains(&name) { continue; }
                    let after: String = chars[i..].iter().take(1).collect();
                    let before: String = if begin > 0 { chars[begin - 1..begin].iter().collect() } else { String::new() };
                    let kind = if after == "(" {
                        CompletionKind::Function
                    } else if before == "." {
                        CompletionKind::Field
                    } else if class_names.contains(&name) {
                        CompletionKind::Class
                    } else if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        CompletionKind::Constant
                    } else {
                        CompletionKind::Variable
                    };
                    let detail = if matches!(kind, CompletionKind::Function) {
                        signature_hint(line, begin)
                    } else { String::new() };
                    upsert(out, &name, kind, detail, true);
                } else { i += 1; }
            }
        }
    }

    /// Workspace-wide identifiers, cached by (root, file count).
    fn workspace_candidates(
        &mut self,
        root: &Path,
        language: Language,
        current: Option<&PathBuf>,
        out: &mut Vec<Candidate>,
    ) {
        let mut files = Vec::new();
        collect_sources(root, &mut files, 0);
        let key = Some((root.to_path_buf(), files.len()));
        if self.workspace_key.as_ref() != key.as_ref() {
            let mut map: HashMap<String, Candidate> = HashMap::new();
            for file in files.iter().take(240) {
                if current.map(|c| c == file).unwrap_or(false) { continue; }
                let Ok(bytes) = std::fs::read(file) else { continue; };
                if bytes.len() > 512 * 1024 || bytes.contains(&0) { continue; }
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines().take(2000) {
                    let chars: Vec<char> = line.chars().collect();
                    let mut i = 0usize;
                    while i < chars.len() {
                        if chars[i].is_alphabetic() || chars[i] == '_' {
                            let begin = i;
                            i += 1;
                            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
                            let name: String = chars[begin..i].iter().collect();
                            if name.len() < 3 { continue; }
                            if language.keywords().contains(&name.as_str()) { continue; }
                            let after: String = chars[i..].iter().take(1).collect();
                            let kind = if after == "(" { CompletionKind::Function }
                                else if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) { CompletionKind::Class }
                                else { CompletionKind::Variable };
                            let entry = map.entry(name.clone()).or_insert_with(|| Candidate {
                                name, kind: kind.clone(), detail: String::new(), local: false, count: 0,
                            });
                            entry.count += 1;
                        } else { i += 1; }
                    }
                }
            }
            self.workspace = map;
            self.workspace_key = key;
        }
        for candidate in self.workspace.values() { out.push(candidate.clone()); }
    }

    /// Best-effort member completion after `.`: infer an object's type from
    /// `let x: Type`, `let x = Type::`, `x = new Type()` then look for methods
    /// declared on that type. Falls back to all method-like identifiers.
    fn member_candidates(&self, chars: &[char], dot: usize, language: Language, out: &mut Vec<Candidate>) {
        let prefix_text: String = chars[..dot].iter().collect();
        let object = prefix_text.chars().rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>().into_iter().rev().collect::<String>();

        let type_name = infer_type(&prefix_text, &object);
        let text: String = chars.iter().collect();
        let lines: Vec<&str> = text.lines().collect();
        for line in &lines {
            let t = line.trim();
            if let Some(ref ty) = type_name {
                // Rust/Java/TS: `impl Type { fn method(` or `Type.prototype.method`
                let marker = format!("{}::", ty);
                if t.contains(&marker) || t.starts_with(&format!("impl {}", ty)) {
                    for word in words(t) {
                        if word.len() > 2 && !language.keywords().contains(&word.as_str()) {
                            upsert(out, &word, CompletionKind::Method, signature_hint(t, t.find(&word).unwrap_or(0)), false);
                        }
                    }
                }
            }
            // Python methods: `def name(self`
            if language == Language::Python && t.starts_with("def ") && t.contains("(self") {
                let name: String = t[4..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    upsert(out, &name, CompletionKind::Method, signature_hint(t, 4), false);
                }
            }
            // Any identifier used after a dot is a plausible member.
            if let Some(pos) = t.find('.') {
                let rest: String = t.chars().skip(pos + 1).take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if rest.len() > 1 { upsert(out, &rest, CompletionKind::Method, String::new(), false); }
            }
        }
    }
}

fn upsert(out: &mut Vec<Candidate>, name: &str, kind: CompletionKind, detail: String, local: bool) {
    if let Some(existing) = out.iter_mut().find(|c| c.name == name) {
        existing.count += 1;
        existing.local |= local;
        if existing.detail.is_empty() { existing.detail = detail; }
        return;
    }
    out.push(Candidate { name: name.to_string(), kind, detail, local, count: 1 });
}

fn words(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let begin = i;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
            out.push(chars[begin..i].iter().collect());
        } else { i += 1; }
    }
    out
}

fn infer_type(prefix: &str, object: &str) -> Option<String> {
    if object.is_empty() { return None; }
    for line in prefix.lines().rev() {
        // `let x: Type` / `x: Type`
        let marker = format!("{}:", object);
        if let Some(pos) = line.find(&marker) {
            let ty: String = line.chars().skip(pos + marker.len())
                .skip_while(|c| c.is_whitespace())
                .take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !ty.is_empty() { return Some(ty); }
        }
        // `let x = Type::` / `x = new Type`
        let marker = format!("{}=", object);
        if let Some(pos) = line.find(&marker) {
            let rest: String = line.chars().skip(pos + marker.len())
                .skip_while(|c| c.is_whitespace())
                .collect();
            for token in words(&rest) {
                if token.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) { return Some(token); }
            }
        }
    }
    None
}

fn signature_hint(line: &str, start: usize) -> String {
    let rest: String = line.chars().skip(start).take(100).collect();
    let end = rest.find(')').map(|i| i + 1).unwrap_or(rest.len().min(60));
    rest[..end].trim().to_string()
}

fn collect_sources(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 || out.len() > 400 { return; }
    let Ok(read) = std::fs::read_dir(root) else { return; };
    for entry in read.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || matches!(name.as_str(), "target" | "node_modules" | "dist") { continue; }
        let path = entry.path();
        if path.is_dir() { collect_sources(&path, out, depth + 1); }
        else if path.extension().map(|e| {
            matches!(e.to_string_lossy().to_lowercase().as_str(),
                "rs"|"js"|"jsx"|"ts"|"tsx"|"py"|"c"|"h"|"cpp"|"cc"|"hpp"|"java"|"go"|"json"|"toml"|"html"|"css")
        }).unwrap_or(false) { out.push(path); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config { Config::default() }

    #[test]
    fn completes_keywords_and_locals() {
        let buffer = Buffer::from_text("fn alpha() {}\nlet beta = 1;\nal", Language::Rust);
        let cursor = buffer.len_chars();
        let mut engine = CompletionEngine::new();
        let state = engine.compute(&buffer, cursor, None, &config(), CompletionContext::Code).unwrap();
        let labels: Vec<&str> = state.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"alpha"), "{:?}", labels);
        assert_eq!(state.replace_end, cursor);

        // Keyword suggestions appear for a keyword prefix.
        let kw = Buffer::from_text("le", Language::Rust);
        let state = engine.compute(&kw, kw.len_chars(), None, &config(), CompletionContext::Code).unwrap();
        assert!(state.items.iter().any(|i| i.label == "let"));
    }

    #[test]
    fn member_access_filters_methods() {
        let buffer = Buffer::from_text("struct Foo;\nimpl Foo { fn measure(&self) {} fn other() {} }\nlet x: Foo = Foo;\nx.measure()\nx.me", Language::Rust);
        let cursor = buffer.len_chars();
        let mut engine = CompletionEngine::new();
        let state = engine.compute(&buffer, cursor, None, &config(), CompletionContext::Code).unwrap();
        assert!(state.items.iter().any(|i| i.label == "measure"));
        assert!(state.items.iter().all(|i| matches!(i.kind, CompletionKind::Method | CompletionKind::Field | CompletionKind::Function)));
    }

    #[test]
    fn no_popup_on_single_character() {
        let buffer = Buffer::from_text("a", Language::Rust);
        let mut engine = CompletionEngine::new();
        assert!(engine.compute(&buffer, 1, None, &config(), CompletionContext::Code).is_none());
    }

    #[test]
    fn tree_sitter_context_suppresses_completion_in_comments() {
        use crate::editor::Highlighter;
        let buffer = Buffer::from_text("// hel", Language::Rust);
        let mut hl = Highlighter::new();
        let _ = hl.spans(&buffer, buffer.revision());
        let context = hl.context_at_byte(buffer.char_to_byte(buffer.len_chars()).saturating_sub(1));
        assert_eq!(context, CompletionContext::Comment);
        let mut engine = CompletionEngine::new();
        assert!(engine.compute(&buffer, buffer.len_chars(), None, &config(), context).is_none());
    }

    #[test]
    fn respects_disabled_setting() {
        let mut cfg = config();
        cfg.autocomplete = false;
        let buffer = Buffer::from_text("let alpha = 1; al", Language::Rust);
        let mut engine = CompletionEngine::new();
        assert!(engine.compute(&buffer, buffer.len_chars(), None, &cfg, CompletionContext::Code).is_none());
    }
}
