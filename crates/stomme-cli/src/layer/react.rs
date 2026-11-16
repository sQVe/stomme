use super::{Refinement, Source};

pub(super) const LAYER: Source = Source {
    name: "react",
    manifest: include_str!("../../../../foundation/layers/react/layer.toml"),
    bans: None,
    refinements: &[Refinement::TsxComponents],
    guide: include_str!("../../../../foundation/layers/react/guide.md"),
};
