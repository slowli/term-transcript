//! Tracing helpers.

use std::{collections::BTreeMap, fmt};

use handlebars::{
    Context, Handlebars, Helper, HelperDef, PathAndJson, RenderContext, RenderError, Renderable,
    ScopedJson, StringOutput,
};
use serde_json::Value as Json;

macro_rules! helper_span {
    ($helper:expr) => {
        tracing::trace_span!(
            target: concat!(env!("CARGO_CRATE_NAME"), "::svg"),
            "call",
            name = $helper.name(),
            params = $crate::svg::helpers::helper_params($helper).map(tracing::field::debug),
            hash = $crate::svg::helpers::helper_hash($helper).map(tracing::field::debug),
        )
        .entered()
    };
}

struct DebugJson<'a>(&'a Json);

impl fmt::Debug for DebugJson<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0, formatter)
    }
}

pub(super) fn helper_params<'h>(helper: &'h Helper<'_>) -> Option<impl fmt::Debug + 'h> {
    struct HelperParams<'rc>(&'rc [PathAndJson<'rc>]);

    impl fmt::Debug for HelperParams<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let items = self.0.iter().map(|param| DebugJson(param.value()));
            formatter.debug_list().entries(items).finish()
        }
    }

    if helper.params().is_empty() {
        None
    } else {
        Some(HelperParams(helper.params()))
    }
}

pub(super) fn helper_hash<'h>(helper: &'h Helper<'_>) -> Option<impl fmt::Debug + 'h> {
    struct HelperHash<'rc>(&'rc BTreeMap<&'rc str, PathAndJson<'rc>>);

    impl fmt::Debug for HelperHash<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let entries = self
                .0
                .iter()
                .map(|(name, value)| (*name, DebugJson(value.value())));
            formatter.debug_map().entries(entries).finish()
        }
    }

    if helper.hash().is_empty() {
        None
    } else {
        Some(HelperHash(helper.hash()))
    }
}

#[derive(Debug)]
pub(super) struct DebugHelper;

impl DebugHelper {
    pub(super) const NAME: &'static str = "debug";
}

impl HelperDef for DebugHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let message = if let Some(template) = helper.template() {
            let mut out = StringOutput::default();
            template.render(reg, ctx, render_ctx, &mut out)?;
            out.into_string()?
        } else {
            String::new()
        };

        let values: BTreeMap<_, _> = helper
            .hash()
            .iter()
            .map(|(name, value)| (*name, DebugJson(value.value())))
            .collect();

        tracing::info!(target: concat!(env!("CARGO_CRATE_NAME"), "::svg"), ?values, "{message}");
        Ok(ScopedJson::Constant(&Json::Null))
    }
}
