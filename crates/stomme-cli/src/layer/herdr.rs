use super::{Detection, Source};

pub(super) const LAYER: Source = Source {
    name: "herdr",
    manifest: include_str!("../../../../foundation/layers/herdr/layer.toml"),
    bans: None,
    refinements: &[],
    detection: Detection::FilesTogether {
        manifest: "mod.json",
        entry: "main.ts",
    },
    guide: include_str!("../../../../foundation/layers/herdr/guide.md"),
};
