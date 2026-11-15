use super::Source;

pub(super) const LAYER: Source = Source {
    name: "gpui",
    manifest: include_str!("../../../../layers/gpui/layer.toml"),
};
