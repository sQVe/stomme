use super::Source;

pub(super) const LAYER: Source = Source {
    name: "pi",
    manifest: include_str!("../../../../layers/pi/layer.toml"),
    bans: None,
};
