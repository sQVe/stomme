use super::{Detection, Refinement, Source};

pub(super) const LAYER: Source = Source {
    name: "react",
    manifest: include_str!("../../../../foundation/layers/react/layer.toml"),
    bans: None,
    refinements: &[Refinement::TsxComponents],
    detection: Detection::PackageDependency("react"),
    guide: include_str!("../../../../foundation/layers/react/guide.md"),
};
