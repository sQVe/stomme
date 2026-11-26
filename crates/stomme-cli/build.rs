use std::env;

#[path = "src/commit.rs"]
mod commit;

const EMBEDDED_COMMIT: &str = "STOMME_EMBEDDED_RELEASE_COMMIT";

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REF");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    // Always emit the key, even when empty: Cargo then overrides any value the build shell
    // exported, so only this script decides whether a commit is embedded.
    let commit = release_commit();
    println!("cargo:rustc-env={EMBEDDED_COMMIT}={commit}");
}

fn release_commit() -> String {
    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let tag = format!("refs/tags/v{version}");
    let reference = env::var("GITHUB_REF").ok();

    if reference.as_deref() != Some(tag.as_str()) {
        return String::new();
    }

    let sha = env::var("GITHUB_SHA")
        .unwrap_or_else(|_| panic!("GITHUB_REF is `{tag}` but GITHUB_SHA is missing"));

    if !commit::is_40_character_lowercase_hex_commit(&sha) {
        panic!("GITHUB_SHA `{sha}` is not a 40-character lowercase hex commit");
    }

    sha
}
