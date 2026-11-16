use super::Source;

pub(super) const LAYER: Source = Source {
    name: "react",
    manifest: include_str!("../../../../layers/react/layer.toml"),
    bans: None,
};
