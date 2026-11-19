use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    C,
    Cpp,
    Java,
    Go,
    Json,
    Html,
    Css,
    Markdown,
    Toml,
    Shell,
    Yaml,
    Sql,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsGrammar {
    Rust,
    JavaScript,
    Python,
}

impl Language {
    pub fn detect(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if name == "cargo.toml" || name == "settings.toml" {
            return Language::Toml;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "rs" => Language::Rust,
            "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" => Language::TypeScript,
            "py" | "pyw" => Language::Python,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Language::Cpp,
            "java" => Language::Java,
            "go" => Language::Go,
            "json" | "jsonc" => Language::Json,
            "html" | "htm" | "vue" | "svg" => Language::Html,
            "css" | "scss" | "less" => Language::Css,
            "md" | "markdown" => Language::Markdown,
            "toml" => Language::Toml,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "yml" | "yaml" => Language::Yaml,
            "sql" => Language::Sql,
            _ => Language::PlainText,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Java => "Java",
            Language::Go => "Go",
            Language::Json => "JSON",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::Markdown => "Markdown",
            Language::Toml => "TOML",
            Language::Shell => "Shell Script",
            Language::Yaml => "YAML",
            Language::Sql => "SQL",
            Language::PlainText => "Plain Text",
        }
    }

    /// Single-column glyphs keep tree alignment intact on terminals that do
    /// not provide Nerd Font glyphs.
    pub fn file_icon(&self) -> &'static str {
        match self {
            Language::Rust => "R",
            Language::JavaScript => "J",
            Language::TypeScript => "T",
            Language::Python => "P",
            Language::C | Language::Cpp => "C",
            Language::Java => "◈",
            Language::Go => "G",
            Language::Json => "{}",
            Language::Html => "<>",
            Language::Css => "#",
            Language::Markdown => "M",
            Language::Toml => "[]",
            Language::Shell => "$_",
            Language::Yaml => "=",
            Language::Sql => "D",
            Language::PlainText => ".",
        }
    }

    pub fn comment_prefix(&self) -> &'static str {
        match self {
            Language::Python | Language::Shell | Language::Yaml => "#",
            Language::Sql => "--",
            Language::Html => "<!--",
            Language::Css => "/*",
            _ => "//",
        }
    }

    pub fn ts_grammar(&self) -> Option<TsGrammar> {
        match self {
            Language::Rust => Some(TsGrammar::Rust),
            Language::JavaScript | Language::TypeScript => Some(TsGrammar::JavaScript),
            Language::Python => Some(TsGrammar::Python),
            _ => None,
        }
    }

    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            Language::Rust => &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "yield"],
            Language::JavaScript | Language::TypeScript => &["async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "export", "extends", "false", "finally", "for", "from", "function", "if", "import", "in", "instanceof", "interface", "let", "new", "null", "of", "return", "static", "super", "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void", "while", "yield"],
            Language::Python => &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while", "with", "yield", "self"],
            Language::C | Language::Cpp => &["auto", "break", "case", "char", "class", "const", "continue", "default", "delete", "do", "double", "else", "enum", "extern", "false", "float", "for", "friend", "goto", "if", "inline", "int", "long", "namespace", "new", "nullptr", "private", "protected", "public", "register", "return", "short", "signed", "sizeof", "static", "struct", "switch", "template", "this", "throw", "true", "try", "typedef", "typename", "union", "unsigned", "using", "virtual", "void", "volatile", "while"],
            Language::Java => &["abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "if", "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private", "protected", "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "true", "false", "null", "try", "void", "volatile", "while"],
            Language::Go => &["break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type", "var", "true", "false", "nil"],
            Language::Json => &["true", "false", "null"],
            Language::Toml => &["true", "false"],
            Language::Shell => &["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "function", "in", "return", "export", "local", "source"],
            Language::Sql => &["SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "ALTER", "DROP", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "AS", "AND", "OR", "NOT", "NULL", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "DISTINCT", "PRIMARY", "KEY", "FOREIGN", "REFERENCES"],
            _ => &[],
        }
    }

    pub fn is_comment(&self, line: &str) -> bool {
        let t = line.trim_start();
        match self.comment_prefix() {
            "<!--" => t.starts_with("<!--"),
            "/*" => t.starts_with("/*") || t.starts_with('*'),
            p => t.starts_with(p),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_extension() {
        assert_eq!(Language::detect(Path::new("src/main.rs")), Language::Rust);
        assert_eq!(Language::detect(Path::new("a.py")), Language::Python);
        assert_eq!(Language::detect(Path::new("a.TS")), Language::TypeScript);
        assert_eq!(Language::detect(Path::new("Makefile")), Language::PlainText);
    }
}
