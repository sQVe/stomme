use super::{Detection, Source};

pub(super) const LAYER: Source = Source {
    name: "gpui-component",
    manifest: include_str!("../../../../foundation/layers/gpui-component/layer.toml"),
    bans: None,
    refinements: &[],
    detection: Detection::CargoPackage("gpui-component"),
    starter: None,
    guide: include_str!("../../../../foundation/layers/gpui-component/guide.md"),
};
