use super::{Detection, Source, Starter};

pub(super) const LAYER: Source = Source {
    name: "herdr",
    manifest: include_str!("../../../../foundation/layers/herdr/layer.toml"),
    bans: None,
    refinements: &[],
    detection: Detection::FilesTogether {
        manifest: "mod.json",
        entry: "main.ts",
    },
    starter: Some(&STARTER),
    guide: include_str!("../../../../foundation/layers/herdr/guide.md"),
};

const STARTER: Starter = Starter {
    files: &[
        (
            "docs/herdr.md",
            include_str!("../../../../foundation/layers/herdr/skeleton/files/docs/herdr.md"),
        ),
        (
            "mods/{{name}}/greeting.test.ts",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/greeting.test.ts"
            ),
        ),
        (
            "mods/{{name}}/greeting.ts",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/greeting.ts"
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
        (
            "mods/{{name}}/tsconfig.json",
            include_str!(
                "../../../../foundation/layers/herdr/skeleton/files/mods/{{name}}/tsconfig.json"
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
