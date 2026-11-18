use super::{Detection, Source, Starter};

pub(super) const LAYER: Source = Source {
    name: "pi",
    manifest: include_str!("../../../../foundation/layers/pi/layer.toml"),
    bans: None,
    refinements: &[],
    detection: Detection::PackageField("pi"),
    starter: Some(&STARTER),
    guide: include_str!("../../../../foundation/layers/pi/guide.md"),
};

const STARTER: Starter = Starter {
    files: &[
        (
            "pnpm-workspace.yaml",
            include_str!("../../../../foundation/layers/pi/skeleton/files/pnpm-workspace.yaml"),
        ),
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
