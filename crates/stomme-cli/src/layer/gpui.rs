use super::{LayerBans, Source};
use crate::rule::Rule;

pub(super) const LAYER: Source = Source {
    name: "gpui",
    manifest: include_str!("../../../../layers/gpui/layer.toml"),
    bans: Some(LayerBans {
        text: include_str!("../../../../layers/gpui/clippy.toml"),
        rule: Rule::GpuiRequiredBan,
    }),
    refinements: &[],
    guide: include_str!("../../../../layers/gpui/guide.md"),
};
