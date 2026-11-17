use super::{InitError, Request};
use crate::layer::Language;

pub(crate) struct Source {
    pub(super) owner: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) merges: &'static [(&'static str, &'static str)],
}

static COMMON: Source = Source {
    owner: "common",
    files: &[],
    merges: &[],
};

static RUST: Source = Source {
    owner: "rust",
    files: &[],
    merges: &[],
};

static TYPESCRIPT: Source = Source {
    owner: "typescript",
    files: &[],
    merges: &[],
};

static GPUI: Source = Source {
    owner: "gpui",
    files: &[],
    merges: &[],
};

static GPUI_COMPONENT: Source = Source {
    owner: "gpui-component",
    files: &[],
    merges: &[],
};

static HERDR: Source = Source {
    owner: "herdr",
    files: &[],
    merges: &[],
};

static PI: Source = Source {
    owner: "pi",
    files: &[],
    merges: &[],
};

static REACT: Source = Source {
    owner: "react",
    files: &[],
    merges: &[],
};

static RUST_CORE: Source = Source {
    owner: "role core",
    files: &[],
    merges: &[],
};

static RUST_UI: Source = Source {
    owner: "role ui",
    files: &[],
    merges: &[],
};

static RUST_KIT: Source = Source {
    owner: "role kit",
    files: &[],
    merges: &[],
};

static RUST_APP: Source = Source {
    owner: "role app",
    files: &[],
    merges: &[],
};

static RUST_CLI: Source = Source {
    owner: "role cli",
    files: &[],
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
