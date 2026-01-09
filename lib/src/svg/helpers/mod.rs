//! Custom Handlebars helpers.

use handlebars::{
    Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, RenderErrorReason,
    ScopedJson,
};
use serde_json::Value as Json;

use self::{
    arith::{OpsHelper, RangeHelper, RoundHelper},
    strings::{CharWidthHelper, LineCounter, LineSplitter, RepeatHelper, TrimHelper},
    vars::{ScopeHelper, SetHelper, SplatVarsHelper},
};

mod arith;
mod strings;
#[cfg(test)]
mod tests;
mod vars;

#[derive(Debug)]
struct TypeofHelper;

impl TypeofHelper {
    const NAME: &'static str = "typeof";
}

impl HelperDef for TypeofHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(helper.params = ?helper.params())
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let val = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let ty = match val.value() {
            Json::Null => "null",
            Json::Bool(_) => "bool",
            Json::Number(_) => "number",
            Json::String(_) => "string",
            Json::Array(_) => "array",
            Json::Object(_) => "object",
        };
        Ok(ScopedJson::Derived(ty.into()))
    }
}

pub(super) fn register_helpers(reg: &mut Handlebars<'_>) {
    // Arithmetic routines
    reg.register_helper("add", Box::new(OpsHelper::Add));
    reg.register_helper("sub", Box::new(OpsHelper::Sub));
    reg.register_helper("mul", Box::new(OpsHelper::Mul));
    reg.register_helper("div", Box::new(OpsHelper::Div));
    reg.register_helper("min", Box::new(OpsHelper::Min));
    reg.register_helper("max", Box::new(OpsHelper::Max));
    reg.register_helper(RoundHelper::NAME, Box::new(RoundHelper));
    reg.register_helper(RangeHelper::NAME, Box::new(RangeHelper));

    // String routines
    reg.register_helper(LineCounter::NAME, Box::new(LineCounter));
    reg.register_helper(LineSplitter::NAME, Box::new(LineSplitter));
    reg.register_helper(RepeatHelper::NAME, Box::new(RepeatHelper));
    reg.register_helper(TrimHelper::NAME, Box::new(TrimHelper));
    reg.register_helper(CharWidthHelper::NAME, Box::new(CharWidthHelper));

    // Introspection helpers
    reg.register_helper(TypeofHelper::NAME, Box::new(TypeofHelper));

    // Variable definition helpers
    reg.register_helper("scope", Box::new(ScopeHelper));
    reg.register_helper(SetHelper::NAME, Box::new(SetHelper));
    reg.register_helper(SplatVarsHelper::NAME, Box::new(SplatVarsHelper));
}
