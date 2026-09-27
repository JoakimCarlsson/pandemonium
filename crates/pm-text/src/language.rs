//! Which language a file is written in, and what that brings with it.
//!
//! A language is the one place a file name turns into everything the editor
//! knows about a file: the grammar its syntax tree is parsed with, the
//! queries its highlights come from, and the language servers that have
//! opinions about it. Nothing else matches on an extension.

use std::fmt::{self, Debug, Formatter};
use std::path::Path;

use tree_sitter::Language as Grammar;
use tree_sitter_language::LanguageFn;

/// A language the editor knows about.
#[derive(Clone, Copy)]
pub struct Language {
    /// The name the status bar shows.
    name: &'static str,
    /// The grammar the syntax tree is parsed with.
    grammar: LanguageFn,
    /// The queries the highlights are captured by, in the order they apply.
    ///
    /// A dialect is its base language's query followed by its own: TypeScript
    /// is JavaScript plus what TypeScript adds, and C++ is C plus what C++
    /// adds, rather than a second copy of either.
    highlights: &'static [&'static str],
    /// The identifier a server is told a document in this language is in.
    language_id: &'static str,
    /// The language servers to run for this language, best first.
    servers: &'static [Server],
    /// What begins a comment that runs to the end of the line, if anything.
    line_comment: Option<&'static str>,
}

/// A language server, as the command that starts one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Server {
    /// The program to run.
    pub command: &'static str,
    /// The arguments to run it with.
    pub arguments: &'static [&'static str],
    /// What the server is configured with as it starts, as a JSON object.
    ///
    /// Some servers keep a feature the editor relies on switched off until
    /// they are told otherwise — gopls colours nothing until it is asked to
    /// — so what a server needs to be useful is named beside how to run it.
    pub options: &'static str,
}

/// The options of a server that needs none.
pub const NO_OPTIONS: &str = "{}";

/// clangd, which serves C and C++ alike.
const CLANGD: &[Server] = &[plain("clangd")];

/// The servers that answer for Python, the strictest type checker first.
const PYTHON_SERVERS: &[Server] = &[
    stdio("basedpyright-langserver"),
    stdio("pyright-langserver"),
    Server {
        command: "ruff",
        arguments: &["server"],
        options: NO_OPTIONS,
    },
    plain("pylsp"),
];

/// The servers that answer for every TypeScript and JavaScript dialect:
/// TypeScript 7's own server, or tsserver or vtsls over an older TypeScript,
/// for the types, ESLint and Biome for the lints, and Tailwind for the class
/// names.
const TSSERVER: &[Server] = &[
    Server {
        command: "tsc",
        arguments: &["--lsp", "--stdio"],
        options: NO_OPTIONS,
    },
    stdio("typescript-language-server"),
    stdio("vtsls"),
    stdio("vscode-eslint-language-server"),
    Server {
        command: "biome",
        arguments: &["lsp-proxy"],
        options: NO_OPTIONS,
    },
    TAILWIND,
];

/// The Tailwind CSS server, which answers wherever class names are written.
const TAILWIND: Server = stdio("tailwindcss-language-server");

/// The servers that answer for C#, csharp-ls first and OmniSharp after it.
const CSHARP_SERVERS: &[Server] = &[
    plain("csharp-ls"),
    Server {
        command: "OmniSharp",
        arguments: &["-lsp"],
        options: NO_OPTIONS,
    },
];

/// The servers that answer for Kotlin, JetBrains' own first.
const KOTLIN_SERVERS: &[Server] = &[stdio("kotlin-lsp"), plain("kotlin-language-server")];

/// The servers that answer for SQL: sqls over any database, Postgres Language
/// Tools over Postgres.
const SQL_SERVERS: &[Server] = &[
    plain("sqls"),
    Server {
        command: "postgrestools",
        arguments: &["lsp-proxy"],
        options: NO_OPTIONS,
    },
];

/// The CSS server VS Code ships, and Tailwind for the class names.
const CSS_SERVER: &[Server] = &[stdio("vscode-css-language-server"), TAILWIND];

/// The HTML server VS Code ships, and Tailwind for the class names.
const HTML_SERVER: &[Server] = &[stdio("vscode-html-language-server"), TAILWIND];

/// The JSON server VS Code ships, which serves JSON and JSONC alike.
const JSON_SERVER: &[Server] = &[stdio("vscode-json-language-server")];

/// yaml-language-server, over YAML.
const YAML_SERVER: &[Server] = &[stdio("yaml-language-server")];

/// The server `name` starts, spoken to over stdio.
const fn stdio(name: &'static str) -> Server {
    Server {
        command: name,
        arguments: &["--stdio"],
        options: NO_OPTIONS,
    }
}

/// The server `name` starts, with no arguments and nothing to configure.
const fn plain(name: &'static str) -> Server {
    Server {
        command: name,
        arguments: &[],
        options: NO_OPTIONS,
    }
}

/// Bash: the grammar, its highlights and bash-language-server.
const BASH: Language = Language {
    name: "Bash",
    language_id: "shellscript",
    grammar: tree_sitter_bash::LANGUAGE,
    highlights: &[tree_sitter_bash::HIGHLIGHT_QUERY],
    servers: &[Server {
        command: "bash-language-server",
        arguments: &["start"],
        options: NO_OPTIONS,
    }],
    line_comment: Some("#"),
};

/// C: the grammar, its highlights and clangd.
const C: Language = Language {
    name: "C",
    language_id: "c",
    grammar: tree_sitter_c::LANGUAGE,
    highlights: &[tree_sitter_c::HIGHLIGHT_QUERY],
    servers: CLANGD,
    line_comment: Some("//"),
};

/// C++: the grammar, C's highlights plus its own, and clangd.
const CPP: Language = Language {
    name: "C++",
    language_id: "cpp",
    grammar: tree_sitter_cpp::LANGUAGE,
    highlights: &[
        tree_sitter_c::HIGHLIGHT_QUERY,
        tree_sitter_cpp::HIGHLIGHT_QUERY,
    ],
    servers: CLANGD,
    line_comment: Some("//"),
};

/// C#: the grammar, its highlights and the C# servers.
const CSHARP: Language = Language {
    name: "C#",
    language_id: "csharp",
    grammar: tree_sitter_c_sharp::LANGUAGE,
    highlights: &[tree_sitter_c_sharp::HIGHLIGHTS_QUERY],
    servers: CSHARP_SERVERS,
    line_comment: Some("//"),
};

/// CSS: the grammar, its highlights and the VS Code CSS server.
const CSS: Language = Language {
    name: "CSS",
    language_id: "css",
    grammar: tree_sitter_css::LANGUAGE,
    highlights: &[tree_sitter_css::HIGHLIGHTS_QUERY],
    servers: CSS_SERVER,
    line_comment: None,
};

/// Dockerfile: the Containerfile grammar, its highlights and the Docker server.
const DOCKERFILE: Language = Language {
    name: "Dockerfile",
    language_id: "dockerfile",
    grammar: tree_sitter_containerfile::LANGUAGE,
    highlights: &[tree_sitter_containerfile::HIGHLIGHTS_QUERY],
    servers: &[stdio("docker-langserver")],
    line_comment: Some("#"),
};

/// Go: the grammar, its highlights and gopls.
const GO: Language = Language {
    name: "Go",
    language_id: "go",
    grammar: tree_sitter_go::LANGUAGE,
    highlights: &[tree_sitter_go::HIGHLIGHTS_QUERY],
    servers: &[Server {
        command: "gopls",
        arguments: &[],
        options: r#"{"semanticTokens": true}"#,
    }],
    line_comment: Some("//"),
};

/// HTML: the grammar, its highlights and the VS Code HTML server.
const HTML: Language = Language {
    name: "HTML",
    language_id: "html",
    grammar: tree_sitter_html::LANGUAGE,
    highlights: &[tree_sitter_html::HIGHLIGHTS_QUERY],
    servers: HTML_SERVER,
    line_comment: None,
};

/// Java: the grammar, its highlights and the Eclipse JDT server.
const JAVA: Language = Language {
    name: "Java",
    language_id: "java",
    grammar: tree_sitter_java::LANGUAGE,
    highlights: &[tree_sitter_java::HIGHLIGHTS_QUERY],
    servers: &[plain("jdtls")],
    line_comment: Some("//"),
};

/// JavaScript: the grammar, its highlights with JSX, and tsserver.
const JAVASCRIPT: Language = Language {
    name: "JavaScript",
    language_id: "javascript",
    grammar: tree_sitter_javascript::LANGUAGE,
    highlights: &[
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
    ],
    servers: TSSERVER,
    line_comment: Some("//"),
};

/// JSX: JavaScript's grammar and highlights with JSX, told apart for tsserver.
///
/// The grammar is the same one plain JavaScript is parsed with, but a server
/// reads a `javascriptreact` document with JSX switched on and a
/// `javascript` one without it.
const JSX: Language = Language {
    name: "JSX",
    language_id: "javascriptreact",
    ..JAVASCRIPT
};

/// JSON: the grammar, its highlights and the VS Code JSON server.
const JSON: Language = Language {
    name: "JSON",
    language_id: "json",
    grammar: tree_sitter_json::LANGUAGE,
    highlights: &[tree_sitter_json::HIGHLIGHTS_QUERY],
    servers: JSON_SERVER,
    line_comment: None,
};

/// JSON with comments: JSON's grammar, told to the server as its own dialect.
const JSONC: Language = Language {
    name: "JSONC",
    language_id: "jsonc",
    grammar: tree_sitter_json::LANGUAGE,
    highlights: &[tree_sitter_json::HIGHLIGHTS_QUERY],
    servers: JSON_SERVER,
    line_comment: Some("//"),
};

/// Kotlin: the grammar, its highlights and the Kotlin servers.
const KOTLIN: Language = Language {
    name: "Kotlin",
    language_id: "kotlin",
    grammar: tree_sitter_kotlin_sg::LANGUAGE,
    highlights: &[tree_sitter_kotlin_sg::HIGHLIGHTS_QUERY],
    servers: KOTLIN_SERVERS,
    line_comment: Some("//"),
};

/// Lua: the grammar, its highlights and lua-language-server.
const LUA: Language = Language {
    name: "Lua",
    language_id: "lua",
    grammar: tree_sitter_lua::LANGUAGE,
    highlights: &[tree_sitter_lua::HIGHLIGHTS_QUERY],
    servers: &[plain("lua-language-server")],
    line_comment: Some("--"),
};

/// Markdown: the block grammar, the highlights of its blocks and marksman.
const MARKDOWN: Language = Language {
    name: "Markdown",
    language_id: "markdown",
    grammar: tree_sitter_md::LANGUAGE,
    highlights: &[tree_sitter_md::HIGHLIGHT_QUERY_BLOCK],
    servers: &[plain("marksman")],
    line_comment: None,
};

/// PHP: the grammar with its embedded HTML, its highlights and Intelephense.
const PHP: Language = Language {
    name: "PHP",
    language_id: "php",
    grammar: tree_sitter_php::LANGUAGE_PHP,
    highlights: &[tree_sitter_php::HIGHLIGHTS_QUERY],
    servers: &[stdio("intelephense")],
    line_comment: Some("//"),
};

/// Python: the grammar, its highlights and the type checkers.
const PYTHON: Language = Language {
    name: "Python",
    language_id: "python",
    grammar: tree_sitter_python::LANGUAGE,
    highlights: &[tree_sitter_python::HIGHLIGHTS_QUERY],
    servers: PYTHON_SERVERS,
    line_comment: Some("#"),
};

/// Ruby: the grammar, its highlights and ruby-lsp.
const RUBY: Language = Language {
    name: "Ruby",
    language_id: "ruby",
    grammar: tree_sitter_ruby::LANGUAGE,
    highlights: &[tree_sitter_ruby::HIGHLIGHTS_QUERY],
    servers: &[plain("ruby-lsp")],
    line_comment: Some("#"),
};

/// Rust: the grammar, its highlights and rust-analyzer.
const RUST: Language = Language {
    name: "Rust",
    language_id: "rust",
    grammar: tree_sitter_rust::LANGUAGE,
    highlights: &[tree_sitter_rust::HIGHLIGHTS_QUERY],
    servers: &[plain("rust-analyzer")],
    line_comment: Some("//"),
};

/// SQL: the grammar, its highlights and the SQL servers.
const SQL: Language = Language {
    name: "SQL",
    language_id: "sql",
    grammar: tree_sitter_sequel::LANGUAGE,
    highlights: &[tree_sitter_sequel::HIGHLIGHTS_QUERY],
    servers: SQL_SERVERS,
    line_comment: Some("--"),
};

/// TOML: the grammar, its highlights and taplo.
const TOML: Language = Language {
    name: "TOML",
    language_id: "toml",
    grammar: tree_sitter_toml_ng::LANGUAGE,
    highlights: &[tree_sitter_toml_ng::HIGHLIGHTS_QUERY],
    servers: &[Server {
        command: "taplo",
        arguments: &["lsp", "stdio"],
        options: NO_OPTIONS,
    }],
    line_comment: Some("#"),
};

/// TSX: the TSX grammar, JavaScript's highlights with JSX plus TypeScript's.
const TSX: Language = Language {
    name: "TSX",
    language_id: "typescriptreact",
    grammar: tree_sitter_typescript::LANGUAGE_TSX,
    highlights: &[
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY,
    ],
    servers: TSSERVER,
    line_comment: Some("//"),
};

/// TypeScript: the grammar, JavaScript's highlights plus its own, and tsserver.
const TYPESCRIPT: Language = Language {
    name: "TypeScript",
    language_id: "typescript",
    grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
    highlights: &[
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY,
    ],
    servers: TSSERVER,
    line_comment: Some("//"),
};

/// YAML: the grammar, its highlights and yaml-language-server.
const YAML: Language = Language {
    name: "YAML",
    language_id: "yaml",
    grammar: tree_sitter_yaml::LANGUAGE,
    highlights: &[tree_sitter_yaml::HIGHLIGHTS_QUERY],
    servers: YAML_SERVER,
    line_comment: Some("#"),
};

/// Every language the editor knows, in the order they are written down.
const KNOWN: &[Language] = &[
    BASH, C, CPP, CSHARP, CSS, DOCKERFILE, GO, HTML, JAVA, JAVASCRIPT, JSX, JSON, JSONC, KOTLIN,
    LUA, MARKDOWN, PHP, PYTHON, RUBY, RUST, SQL, TOML, TSX, TYPESCRIPT, YAML,
];

impl Debug for Language {
    /// Writes the language's name, the grammar behind it being a pointer.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

impl Language {
    /// The language a file at `path` is written in.
    ///
    /// A whole file name decides first — a dotfile has no extension, and
    /// `tsconfig.json` is JSON with comments however it is spelled — and the
    /// extension decides the rest.
    pub fn of(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if let Some(language) = Self::named(name) {
            return Some(language);
        }
        Self::extended(path.extension()?.to_str()?)
    }

    /// The language a fenced block of markdown tagged `tag` is written in.
    ///
    /// A fence is tagged by whoever wrote it, which is sometimes a language's
    /// name and sometimes its usual extension, so both are accepted.
    pub fn fenced(tag: &str) -> Option<Self> {
        let tag = tag.trim().to_ascii_lowercase();
        let named = match tag.as_str() {
            "bash" | "shell" | "shellscript" | "zsh" => Some(BASH),
            "c++" | "cpp" => Some(CPP),
            "c#" | "csharp" => Some(CSHARP),
            "golang" => Some(GO),
            "javascript" => Some(JAVASCRIPT),
            "javascriptreact" => Some(JSX),
            "kotlin" => Some(KOTLIN),
            "python" => Some(PYTHON),
            "ruby" => Some(RUBY),
            "rust" => Some(RUST),
            "typescript" => Some(TYPESCRIPT),
            "typescriptreact" => Some(TSX),
            _ => None,
        };
        named.or_else(|| Self::extended(&tag))
    }

    /// The language a file called `name` is written in, when its whole name
    /// says so rather than its extension.
    fn named(name: &str) -> Option<Self> {
        match name {
            ".bashrc" | ".bash_profile" | ".profile" | ".zshrc" | ".zprofile" => Some(BASH),
            "tsconfig.json" | "jsconfig.json" => Some(JSONC),
            "Containerfile" | "Dockerfile" => Some(DOCKERFILE),
            "Gemfile" | "Rakefile" => Some(RUBY),
            _ => None,
        }
    }

    /// The language a file with extension `extension` is written in.
    fn extended(extension: &str) -> Option<Self> {
        match extension {
            "sh" | "bash" | "zsh" | "ksh" => Some(BASH),
            "c" | "h" => Some(C),
            "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Some(CPP),
            "cs" | "csx" => Some(CSHARP),
            "css" | "scss" => Some(CSS),
            "containerfile" | "dockerfile" => Some(DOCKERFILE),
            "go" => Some(GO),
            "htm" | "html" => Some(HTML),
            "java" => Some(JAVA),
            "cjs" | "js" | "mjs" => Some(JAVASCRIPT),
            "jsx" => Some(JSX),
            "json" => Some(JSON),
            "jsonc" => Some(JSONC),
            "kt" | "kts" => Some(KOTLIN),
            "lua" => Some(LUA),
            "md" | "markdown" => Some(MARKDOWN),
            "php" => Some(PHP),
            "py" | "pyi" => Some(PYTHON),
            "gemspec" | "rake" | "rb" => Some(RUBY),
            "rs" => Some(RUST),
            "sql" => Some(SQL),
            "toml" => Some(TOML),
            "tsx" => Some(TSX),
            "cts" | "mts" | "ts" => Some(TYPESCRIPT),
            "yaml" | "yml" => Some(YAML),
            _ => None,
        }
    }

    /// The name the status bar shows.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The language `name` asks for, by its status-bar name or the identifier
    /// a server is told.
    #[must_use]
    pub fn called(name: &str) -> Option<Self> {
        KNOWN.iter().copied().find(|language| {
            language.name.eq_ignore_ascii_case(name)
                || language.language_id.eq_ignore_ascii_case(name)
        })
    }

    /// The identifier a server is told a document in this language is in.
    pub const fn language_id(self) -> &'static str {
        self.language_id
    }

    /// The grammar the syntax tree is parsed with.
    pub fn grammar(self) -> Grammar {
        Grammar::new(self.grammar)
    }

    /// The queries the highlights are captured by, as one query.
    pub fn highlights(self) -> String {
        self.highlights.join("\n")
    }

    /// The language servers to run for this language, best first.
    pub const fn servers(self) -> &'static [Server] {
        self.servers
    }

    /// What begins a comment that runs to the end of the line, if anything.
    pub const fn line_comment(self) -> Option<&'static str> {
        self.line_comment
    }
}
