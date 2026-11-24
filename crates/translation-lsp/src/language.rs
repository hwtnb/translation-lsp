//! The languages that `extension.toml` registers for the server.

/// A language that `extension.toml` registers, named by its language server ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Language {
    Rust,
    JavaScript,
    TypeScript,
    TypeScriptReact,
    CSharp,
    C,
    Cpp,
    Python,
    Go,
    Java,
    Kotlin,
    Swift,
    Ruby,
    Php,
    Html,
    Vue,
    Svelte,
    Astro,
    Xml,
    Css,
    Scss,
    Sql,
    Lua,
    Zig,
    Proto,
    GraphQl,
    Clojure,
    Haskell,
    OCaml,
    OCamlInterface,
    Scala,
    Dart,
    Elixir,
    Terraform,
    TerraformVars,
    Hcl,
    Nix,
    Prisma,
    Gleam,
    Erlang,
    CMake,
    Glsl,
    FSharp,
    Lean,
    Json,
    Jsonc,
    Yaml,
    Toml,
    ShellScript,
    PowerShell,
    Dockerfile,
    DockerCompose,
    Make,
    Markdown,
    PlainText,
}

impl Language {
    /// Every language, in the order of `extension.toml`.
    pub const ALL: [Self; 55] = [
        Self::Rust,
        Self::JavaScript,
        Self::TypeScript,
        Self::TypeScriptReact,
        Self::CSharp,
        Self::C,
        Self::Cpp,
        Self::Python,
        Self::Go,
        Self::Java,
        Self::Kotlin,
        Self::Swift,
        Self::Ruby,
        Self::Php,
        Self::Html,
        Self::Vue,
        Self::Svelte,
        Self::Astro,
        Self::Xml,
        Self::Css,
        Self::Scss,
        Self::Sql,
        Self::Lua,
        Self::Zig,
        Self::Proto,
        Self::GraphQl,
        Self::Clojure,
        Self::Haskell,
        Self::OCaml,
        Self::OCamlInterface,
        Self::Scala,
        Self::Dart,
        Self::Elixir,
        Self::Terraform,
        Self::TerraformVars,
        Self::Hcl,
        Self::Nix,
        Self::Prisma,
        Self::Gleam,
        Self::Erlang,
        Self::CMake,
        Self::Glsl,
        Self::FSharp,
        Self::Lean,
        Self::Json,
        Self::Jsonc,
        Self::Yaml,
        Self::Toml,
        Self::ShellScript,
        Self::PowerShell,
        Self::Dockerfile,
        Self::DockerCompose,
        Self::Make,
        Self::Markdown,
        Self::PlainText,
    ];

    /// The language ID that Zed sends and that settings name.
    pub fn id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::TypeScriptReact => "typescriptreact",
            Self::CSharp => "csharp",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Python => "python",
            Self::Go => "go",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Html => "html",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
            Self::Xml => "xml",
            Self::Css => "css",
            Self::Scss => "scss",
            Self::Sql => "sql",
            Self::Lua => "lua",
            Self::Zig => "zig",
            Self::Proto => "proto",
            Self::GraphQl => "graphql",
            Self::Clojure => "clojure",
            Self::Haskell => "haskell",
            Self::OCaml => "ocaml",
            Self::OCamlInterface => "ocaml.interface",
            Self::Scala => "scala",
            Self::Dart => "dart",
            Self::Elixir => "elixir",
            Self::Terraform => "terraform",
            Self::TerraformVars => "terraform-vars",
            Self::Hcl => "hcl",
            Self::Nix => "nix",
            Self::Prisma => "prisma",
            Self::Gleam => "gleam",
            Self::Erlang => "erlang",
            Self::CMake => "cmake",
            Self::Glsl => "glsl",
            Self::FSharp => "fsharp",
            Self::Lean => "lean4",
            Self::Json => "json",
            Self::Jsonc => "jsonc",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::ShellScript => "shellscript",
            Self::PowerShell => "powershell",
            Self::Dockerfile => "dockerfile",
            Self::DockerCompose => "dockercompose",
            Self::Make => "makefile",
            Self::Markdown => "markdown",
            Self::PlainText => "plaintext",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.id() == id)
    }

    /// The language of a document that the editor opens with language ID
    /// `id`. Some VS Code extensions send language IDs that settings do not
    /// name; those map to the corresponding language here so its comment
    /// reader and settings apply.
    pub fn of_document(id: &str) -> Option<Self> {
        match id {
            "javascriptreact" => Some(Self::JavaScript),
            "proto3" => Some(Self::Proto),
            "kotlinscript" => Some(Self::Kotlin),
            "terraform-stack"
            | "terraform-deploy"
            | "terraform-test"
            | "terraform-mock"
            | "terraform-search"
            | "terraform-policy"
            | "terraform-policytest" => Some(Self::Hcl),
            id => Self::from_id(id),
        }
    }

    /// Markdown and Plain Text hold prose, where the other languages hold code.
    pub fn is_prose(self) -> bool {
        matches!(self, Self::Markdown | Self::PlainText)
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn javascriptreact_maps_to_javascript() {
        assert_eq!(
            Language::of_document("javascriptreact"),
            Some(Language::JavaScript)
        );
        assert_eq!(Language::from_id("javascriptreact"), None);
    }

    #[test]
    fn proto3_maps_to_proto() {
        assert_eq!(Language::of_document("proto3"), Some(Language::Proto));
        assert_eq!(Language::from_id("proto3"), None);
    }

    #[test]
    fn kotlinscript_maps_to_kotlin() {
        assert_eq!(
            Language::of_document("kotlinscript"),
            Some(Language::Kotlin)
        );
        assert_eq!(Language::from_id("kotlinscript"), None);
    }

    #[test]
    fn terraform_stack_maps_to_hcl() {
        assert_eq!(
            Language::of_document("terraform-stack"),
            Some(Language::Hcl)
        );
        assert_eq!(Language::from_id("terraform-stack"), None);
    }

    #[test]
    fn terraform_deploy_maps_to_hcl() {
        assert_eq!(
            Language::of_document("terraform-deploy"),
            Some(Language::Hcl)
        );
        assert_eq!(Language::from_id("terraform-deploy"), None);
    }

    #[test]
    fn terraform_test_maps_to_hcl() {
        assert_eq!(Language::of_document("terraform-test"), Some(Language::Hcl));
        assert_eq!(Language::from_id("terraform-test"), None);
    }

    #[test]
    fn terraform_mock_maps_to_hcl() {
        assert_eq!(Language::of_document("terraform-mock"), Some(Language::Hcl));
        assert_eq!(Language::from_id("terraform-mock"), None);
    }

    #[test]
    fn terraform_search_maps_to_hcl() {
        assert_eq!(
            Language::of_document("terraform-search"),
            Some(Language::Hcl)
        );
        assert_eq!(Language::from_id("terraform-search"), None);
    }

    #[test]
    fn terraform_policy_maps_to_hcl() {
        assert_eq!(
            Language::of_document("terraform-policy"),
            Some(Language::Hcl)
        );
        assert_eq!(Language::from_id("terraform-policy"), None);
    }

    #[test]
    fn terraform_policytest_maps_to_hcl() {
        assert_eq!(
            Language::of_document("terraform-policytest"),
            Some(Language::Hcl)
        );
        assert_eq!(Language::from_id("terraform-policytest"), None);
    }
}
