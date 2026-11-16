use super::Source;

pub(super) const LAYER: Source = Source {
    name: "herdr",
    manifest: include_str!("../../../../layers/herdr/layer.toml"),
    bans: None,
    refinements: &[],
    guide: include_str!("../../../../layers/herdr/guide.md"),
};
