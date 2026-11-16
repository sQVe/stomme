use super::Source;

pub(super) const LAYER: Source = Source {
    name: "gpui-component",
    manifest: include_str!("../../../../layers/gpui-component/layer.toml"),
    bans: None,
    refinements: &[],
    guide: include_str!("../../../../layers/gpui-component/guide.md"),
};
