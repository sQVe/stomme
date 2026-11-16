use super::{LayerBans, Source};
use crate::rule::Rule;

pub(super) const LAYER: Source = Source {
    name: "gpui",
    manifest: include_str!("../../../../foundation/layers/gpui/layer.toml"),
    bans: Some(LayerBans {
        text: include_str!("../../../../foundation/layers/gpui/clippy.toml"),
        rule: Rule::GpuiRequiredBan,
    }),
    refinements: &[],
    guide: include_str!("../../../../foundation/layers/gpui/guide.md"),
};
