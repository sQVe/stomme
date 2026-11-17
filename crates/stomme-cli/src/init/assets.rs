use super::{InitError, Request};
use crate::layer::Language;

pub(crate) struct Source {
    pub(super) owner: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) merges: &'static [(&'static str, &'static str)],
}

static COMMON: Source = Source {
    owner: "common",
    files: &[
        (
            "LICENSE",
            include_str!("../../../../foundation/common/skeleton/files/LICENSE"),
        ),
        (
            "AGENTS.md",
            include_str!("../../../../foundation/common/skeleton/files/AGENTS.md"),
        ),
        (
            "docs/README.md",
            include_str!("../../../../foundation/common/skeleton/files/docs/README.md"),
        ),
        (
            "docs/adr/README.md",
            include_str!("../../../../foundation/common/skeleton/files/docs/adr/README.md"),
        ),
        (
            "docs/adr/TEMPLATE.md",
            include_str!("../../../../foundation/common/skeleton/files/docs/adr/TEMPLATE.md"),
        ),
        (
            "renovate.json",
            include_str!("../../../../foundation/common/skeleton/files/renovate.json"),
        ),
        (
            ".github/workflows/sync-update.yml",
            include_str!(
                "../../../../foundation/common/skeleton/files/.github/workflows/sync-update.yml"
            ),
        ),
    ],
    merges: &[],
};

static RUST: Source = Source {
    owner: "rust",
    files: &[
        (
            ".cargo/config.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/files/.cargo/config.toml"),
        ),
        (
            ".github/workflows/ci.yml",
            include_str!(
                "../../../../foundation/languages/rust/skeleton/files/.github/workflows/ci.yml"
            ),
        ),
        (
            ".gitignore",
            include_str!("../../../../foundation/languages/rust/skeleton/files/.gitignore"),
        ),
        (
            ".pi/tau.json",
            include_str!("../../../../foundation/languages/rust/skeleton/files/.pi/tau.json"),
        ),
        (
            "crates/{{name}}/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/skeleton/files/crates/{{name}}/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/skeleton/files/crates/{{name}}/src/lib.rs"
            ),
        ),
        (
            "docs/development.md",
            include_str!(
                "../../../../foundation/languages/rust/skeleton/files/docs/development.md"
            ),
        ),
        (
            "mise.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/files/mise.toml"),
        ),
        (
            "rust-toolchain.toml",
            include_str!(
                "../../../../foundation/languages/rust/skeleton/files/rust-toolchain.toml"
            ),
        ),
        (
            "rustfmt.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/files/rustfmt.toml"),
        ),
        (
            "xtask/Cargo.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/files/xtask/Cargo.toml"),
        ),
        (
            "xtask/src/main.rs",
            include_str!("../../../../foundation/languages/rust/skeleton/files/xtask/src/main.rs"),
        ),
    ],
    merges: &[
        (
            "Cargo.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/merge/Cargo.toml"),
        ),
        (
            "clippy.toml",
            include_str!("../../../../foundation/languages/rust/skeleton/merge/clippy.toml"),
        ),
    ],
};

static TYPESCRIPT: Source = Source {
    owner: "typescript",
    files: &[
        (
            ".github/workflows/ci.yml",
            include_str!(
                "../../../../foundation/languages/typescript/skeleton/files/.github/workflows/ci.yml"
            ),
        ),
        (
            ".gitignore",
            include_str!("../../../../foundation/languages/typescript/skeleton/files/.gitignore"),
        ),
        (
            ".pi/tau.json",
            include_str!("../../../../foundation/languages/typescript/skeleton/files/.pi/tau.json"),
        ),
        (
            "docs/development.md",
            include_str!(
                "../../../../foundation/languages/typescript/skeleton/files/docs/development.md"
            ),
        ),
        (
            "src/main.test.ts",
            include_str!(
                "../../../../foundation/languages/typescript/skeleton/files/src/main.test.ts"
            ),
        ),
        (
            "src/main.ts",
            include_str!("../../../../foundation/languages/typescript/skeleton/files/src/main.ts"),
        ),
        (
            "tsconfig.json",
            include_str!(
                "../../../../foundation/languages/typescript/skeleton/files/tsconfig.json"
            ),
        ),
        (
            "vite.config.ts",
            include_str!(
                "../../../../foundation/languages/typescript/skeleton/files/vite.config.ts"
            ),
        ),
    ],
    merges: &[
        (
            "mise.toml",
            include_str!("../../../../foundation/languages/typescript/skeleton/merge/mise.toml"),
        ),
        (
            "package.json",
            include_str!("../../../../foundation/languages/typescript/skeleton/merge/package.json"),
        ),
    ],
};

static GPUI: Source = Source {
    owner: "gpui",
    files: &[],
    merges: &[(
        "clippy.toml",
        include_str!("../../../../foundation/layers/gpui/skeleton/merge/clippy.toml"),
    )],
};

static GPUI_COMPONENT: Source = Source {
    owner: "gpui-component",
    files: &[],
    merges: &[],
};

static HERDR: Source = Source {
    owner: "herdr",
    files: &[
        (
            "mods/{{name}}/main.test.ts",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/main.test.ts"
            ),
        ),
        (
            "mods/{{name}}/main.ts",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/main.ts"
            ),
        ),
        (
            "mods/{{name}}/mod.json",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/mod.json"
            ),
        ),
    ],
    merges: &[
        (
            "mise.toml",
            include_str!("../../../../foundation/layers/herdr/skeleton/merge/mise.toml"),
        ),
        (
            "package.json",
            include_str!("../../../../foundation/layers/herdr/skeleton/merge/package.json"),
        ),
    ],
};

static PI: Source = Source {
    owner: "pi",
    files: &[
        (
            "src/piExtension.test.ts",
            include_str!("../../../../foundation/layers/pi/skeleton/files/src/piExtension.test.ts"),
        ),
        (
            "src/piExtension.ts",
            include_str!("../../../../foundation/layers/pi/skeleton/files/src/piExtension.ts"),
        ),
    ],
    merges: &[(
        "package.json",
        include_str!("../../../../foundation/layers/pi/skeleton/merge/package.json"),
    )],
};

static REACT: Source = Source {
    owner: "react",
    files: &[(
        "src/App.tsx",
        include_str!("../../../../foundation/layers/react/skeleton/files/src/App.tsx"),
    )],
    merges: &[(
        "package.json",
        include_str!("../../../../foundation/layers/react/skeleton/merge/package.json"),
    )],
};

static RUST_CORE: Source = Source {
    owner: "role core",
    files: &[
        (
            "crates/{{name}}-core/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/roles/core/skeleton/files/crates/{{name}}-core/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}-core/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/roles/core/skeleton/files/crates/{{name}}-core/src/lib.rs"
            ),
        ),
    ],
    merges: &[],
};

static RUST_UI: Source = Source {
    owner: "role ui",
    files: &[
        (
            "crates/{{name}}-ui/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/roles/ui/skeleton/files/crates/{{name}}-ui/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}-ui/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/roles/ui/skeleton/files/crates/{{name}}-ui/src/lib.rs"
            ),
        ),
    ],
    merges: &[],
};

static RUST_KIT: Source = Source {
    owner: "role kit",
    files: &[
        (
            "crates/{{name}}-kit/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/roles/kit/skeleton/files/crates/{{name}}-kit/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}-kit/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/roles/kit/skeleton/files/crates/{{name}}-kit/src/lib.rs"
            ),
        ),
    ],
    merges: &[],
};

static RUST_APP: Source = Source {
    owner: "role app",
    files: &[
        (
            "crates/{{name}}-app/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/roles/app/skeleton/files/crates/{{name}}-app/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}-app/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/roles/app/skeleton/files/crates/{{name}}-app/src/lib.rs"
            ),
        ),
    ],
    merges: &[],
};

static RUST_CLI: Source = Source {
    owner: "role cli",
    files: &[
        (
            "crates/{{name}}-cli/Cargo.toml",
            include_str!(
                "../../../../foundation/languages/rust/roles/cli/skeleton/files/crates/{{name}}-cli/Cargo.toml"
            ),
        ),
        (
            "crates/{{name}}-cli/src/lib.rs",
            include_str!(
                "../../../../foundation/languages/rust/roles/cli/skeleton/files/crates/{{name}}-cli/src/lib.rs"
            ),
        ),
    ],
    merges: &[],
};

pub(crate) fn sources(request: &Request) -> Result<Vec<&'static Source>, InitError> {
    let mut sources = vec![&COMMON, language(request.language)];
    let mut layers: Vec<&str> = request.layers.iter().map(String::as_str).collect();
    let mut roles: Vec<&str> = request.roles.iter().map(String::as_str).collect();

    layers.sort_unstable();
    roles.sort_unstable();
    sources.extend(layers.into_iter().filter_map(layer));

    for name in roles {
        let source = role(request.language, name).ok_or_else(|| InitError::UnknownRole {
            role: name.to_string(),
            language: request.language,
        })?;

        sources.push(source);
    }

    Ok(sources)
}

pub(crate) fn language(language: Language) -> &'static Source {
    match language {
        Language::Rust => &RUST,
        Language::TypeScript => &TYPESCRIPT,
    }
}

pub(crate) fn layer(name: &str) -> Option<&'static Source> {
    match name {
        "gpui" => Some(&GPUI),
        "gpui-component" => Some(&GPUI_COMPONENT),
        "herdr" => Some(&HERDR),
        "pi" => Some(&PI),
        "react" => Some(&REACT),
        _ => None,
    }
}

pub(crate) fn role(language: Language, name: &str) -> Option<&'static Source> {
    match (language, name) {
        (Language::Rust, "core") => Some(&RUST_CORE),
        (Language::Rust, "ui") => Some(&RUST_UI),
        (Language::Rust, "kit") => Some(&RUST_KIT),
        (Language::Rust, "app") => Some(&RUST_APP),
        (Language::Rust, "cli") => Some(&RUST_CLI),
        _ => None,
    }
}
