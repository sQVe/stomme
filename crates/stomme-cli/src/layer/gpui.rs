use super::{Detection, LayerBans, Source, Starter};
use crate::rule::Rule;

pub(super) const LAYER: Source = Source {
    name: "gpui",
    manifest: include_str!("../../../../foundation/layers/gpui/layer.toml"),
    bans: Some(LayerBans {
        text: include_str!("../../../../foundation/layers/gpui/clippy.toml"),
        rule: Rule::GpuiRequiredBan,
    }),
    refinements: &[],
    detection: Detection::CargoPackage("gpui"),
    starter: Some(&STARTER),
    guide: include_str!("../../../../foundation/layers/gpui/guide.md"),
};

const STARTER: Starter = Starter {
    files: &[],
    merges: &[(
        "clippy.toml",
        include_str!("../../../../foundation/layers/gpui/skeleton/merge/clippy.toml"),
    )],
};
