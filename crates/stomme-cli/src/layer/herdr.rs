use super::Source;

pub(super) const LAYER: Source = Source {
    name: "herdr",
    manifest: include_str!("../../../../foundation/layers/herdr/layer.toml"),
    bans: None,
    refinements: &[],
    guide: include_str!("../../../../foundation/layers/herdr/guide.md"),
};
