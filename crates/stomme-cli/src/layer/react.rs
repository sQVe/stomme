use super::{Detection, Refinement, Source, Starter};

pub(super) const LAYER: Source = Source {
    name: "react",
    manifest: include_str!("../../../../foundation/layers/react/layer.toml"),
    bans: None,
    refinements: &[Refinement::TsxComponents],
    detection: Detection::PackageDependency("react"),
    starter: Some(&STARTER),
    guide: include_str!("../../../../foundation/layers/react/guide.md"),
};

const STARTER: Starter = Starter {
    files: &[(
        "src/App.tsx",
        include_str!("../../../../foundation/layers/react/skeleton/files/src/App.tsx"),
    )],
    merges: &[(
        "package.json",
        include_str!("../../../../foundation/layers/react/skeleton/merge/package.json"),
    )],
};
