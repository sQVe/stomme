use super::Source;

pub(super) const LAYER: Source = Source {
    name: "pi",
    manifest: include_str!("../../../../foundation/layers/pi/layer.toml"),
    bans: None,
    refinements: &[],
    guide: include_str!("../../../../foundation/layers/pi/guide.md"),
};
