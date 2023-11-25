//! Custom Handlebars helpers.

use handlebars::{
    BlockContext, Context, Handlebars, Helper, HelperDef, Output, RenderContext, RenderError,
    Renderable, ScopedJson, StringOutput,
};
use serde_json::Value as Json;

use std::sync::Mutex;

/// Tries to convert an `f64` number to `i64` without precision loss.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn to_i64(value: f64) -> Option<i64> {
    const MAX_ACCURATE_VALUE: f64 = (1_i64 << 53) as f64;
    const MIN_ACCURATE_VALUE: f64 = -(1_i64 << 53) as f64;

    if (MIN_ACCURATE_VALUE..=MAX_ACCURATE_VALUE).contains(&value) {
        Some(value as i64)
    } else {
        None
    }
}

#[derive(Debug)]
struct ScopeHelper;

impl HelperDef for ScopeHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", skip_all, err, fields(helper.hash = ?helper.hash()))
    )]
    fn call<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'reg, 'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        const MESSAGE: &str = "`scope` must be called as block helper";

        let template = helper.template().ok_or_else(|| RenderError::new(MESSAGE))?;
        if !helper.params().is_empty() {
            return Err(RenderError::new(MESSAGE));
        }

        for (name, value) in helper.hash() {
            let helper = VarHelper::new(value.value().clone());
            render_ctx.register_local_helper(name, Box::new(helper));
        }

        let result = template.render(reg, ctx, render_ctx, out);
        for name in helper.hash().keys() {
            render_ctx.unregister_local_helper(name);
        }
        result
    }
}

#[derive(Debug)]
struct VarHelper {
    value: Mutex<Json>,
}

impl VarHelper {
    fn new(value: Json) -> Self {
        Self {
            value: Mutex::new(value),
        }
    }

    fn set_value(&self, value: Json) {
        #[cfg(feature = "tracing")]
        tracing::trace!(?value, "overwritten var");
        *self.value.lock().unwrap() = value;
    }
}

impl HelperDef for VarHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(
                self = ?self,
                helper.name = helper.name(),
                helper.is_block = helper.is_block(),
                helper.set = ?helper.hash_get("set")
            )
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'reg, 'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        if helper.is_block() {
            if !helper.params().is_empty() {
                let message = "In block form, var helpers must be called without args";
                return Err(RenderError::new(message));
            }

            let value = if let Some(template) = helper.template() {
                let mut output = StringOutput::new();
                template.render(reg, ctx, render_ctx, &mut output)?;
                let json_string = output.into_string()?;
                serde_json::from_str(&json_string).map_err(|err| {
                    let message = format!("Cannot parse JSON value: {err}");
                    RenderError::new(message)
                })?
            } else {
                Json::Null
            };

            self.set_value(value);
            Ok(ScopedJson::Constant(&Json::Null))
        } else {
            if !helper.params().is_empty() {
                let message = "variable helper misuse; should be called without args";
                return Err(RenderError::new(message));
            }

            if let Some(value) = helper.hash_get("set") {
                // Variable setter.
                self.set_value(value.value().clone());
                Ok(ScopedJson::Constant(&Json::Null))
            } else {
                // Variable getter.
                let value = self.value.lock().unwrap().clone();
                Ok(ScopedJson::Derived(value))
            }
        }
    }
}

#[derive(Debug)]
enum OpsHelper {
    Add,
    Mul,
    Sub,
    Div,
}

impl OpsHelper {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Sub => "sub",
            Self::Div => "div",
        }
    }

    fn accumulate_i64(&self, mut values: impl Iterator<Item = i64>) -> i64 {
        match self {
            Self::Add => values.sum(),
            Self::Mul => values.product(),
            // `unwrap`s are safe because of previous checks
            Self::Sub => values.next().unwrap() - values.next().unwrap(),
            Self::Div => unreachable!(),
        }
    }

    fn accumulate_f64(&self, mut values: impl Iterator<Item = f64>) -> f64 {
        match self {
            Self::Add => values.sum(),
            Self::Mul => values.product(),
            // `unwrap`s are safe because of previous checks
            Self::Sub => values.next().unwrap() - values.next().unwrap(),
            Self::Div => values.next().unwrap() / values.next().unwrap(),
        }
    }
}

impl HelperDef for OpsHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(
                self = ?self,
                helper.name = helper.name(),
                helper.params = ?helper.params(),
                helper.round = ?helper.hash_get("round")
            )
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        if matches!(self, Self::Sub | Self::Div) && helper.params().len() != 2 {
            let message = format!("`{}` expects exactly 2 number args", self.as_str());
            return Err(RenderError::new(message));
        }

        if !matches!(self, Self::Div) {
            let all_ints = helper.params().iter().all(|param| param.value().is_i64());
            #[cfg(feature = "tracing")]
            tracing::trace!(all_ints, "checked if all numbers are ints");

            if all_ints {
                let values = helper
                    .params()
                    .iter()
                    .map(|param| param.value().as_i64().unwrap());
                let acc = self.accumulate_i64(values);
                return Ok(ScopedJson::Derived(acc.into()));
            }
        }

        let all_floats = helper
            .params()
            .iter()
            .all(|param| param.value().as_f64().is_some());
        if all_floats {
            let values = helper
                .params()
                .iter()
                .map(|param| param.value().as_f64().unwrap());
            let mut acc = self.accumulate_f64(values);
            let acc: Json = if let Some(rounding) = helper.hash_get("round") {
                if matches!(rounding.value(), Json::Bool(true)) {
                    acc = acc.round();
                } else if rounding.value().as_str() == Some("up") {
                    acc = acc.ceil();
                } else if rounding.value().as_str() == Some("down") {
                    acc = acc.floor();
                }
                // Try to present the value as `i64` (this could be beneficial for other helpers).
                // If this doesn't work, present it as an original floating-point value.
                to_i64(acc).map_or_else(|| acc.into(), Into::into)
            } else {
                acc.into()
            };
            Ok(ScopedJson::Derived(acc))
        } else {
            let message = "all args must be numbers";
            Err(RenderError::new(message))
        }
    }
}

#[derive(Debug)]
struct EvalHelper;

impl HelperDef for EvalHelper {
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
        helper: &Helper<'reg, 'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        const MESSAGE: &str = "`eval` must be called with partial name as first arg";

        let partial_name = helper.param(0).ok_or_else(|| RenderError::new(MESSAGE))?;
        let partial_name = partial_name
            .value()
            .as_str()
            .ok_or_else(|| RenderError::new(MESSAGE))?;

        let partial = render_ctx.get_partial(partial_name).ok_or_else(|| {
            let message = format!("partial `{partial_name}` not found");
            RenderError::new(message)
        })?;

        let object: serde_json::Map<String, Json> = helper
            .hash()
            .iter()
            .map(|(&name, value)| (name.to_owned(), value.value().clone()))
            .collect();

        let mut render_ctx = render_ctx.clone();
        while render_ctx.block().is_some() {
            render_ctx.pop_block();
        }
        let mut block_ctx = BlockContext::new();
        block_ctx.set_base_value(Json::from(object));
        render_ctx.push_block(block_ctx);

        let mut output = StringOutput::new();
        partial.render(reg, ctx, &mut render_ctx, &mut output)?;
        let json_string = output.into_string()?;
        let json: Json = serde_json::from_str(&json_string).map_err(|err| {
            let message = format!("Cannot parse JSON value: {err}");
            RenderError::new(message)
        })?;
        Ok(ScopedJson::Derived(json))
    }
}

#[derive(Debug)]
struct LineCounter;

impl HelperDef for LineCounter {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(helper.params = ?helper.params(), helper.format = ?helper.hash_get("format"))
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        let string = helper
            .param(0)
            .ok_or_else(|| RenderError::new("must be called with a single arg"))?;
        let string = string
            .value()
            .as_str()
            .ok_or_else(|| RenderError::new("argument must be a string"))?;
        let is_html = helper
            .hash_get("format")
            .map_or(false, |format| format.value().as_str() == Some("html"));

        let mut lines = bytecount::count(string.as_bytes(), b'\n');
        if is_html {
            lines += string.matches("<br/>").count();
        }
        if !string.is_empty() && !string.ends_with('\n') {
            lines += 1;
        }

        let lines = u64::try_from(lines)
            .map_err(|err| RenderError::new(format!("cannot convert length: {err}")))?;
        Ok(ScopedJson::Derived(lines.into()))
    }
}

#[derive(Debug)]
struct LineSplitter;

impl HelperDef for LineSplitter {
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
        helper: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        let string = helper
            .param(0)
            .ok_or_else(|| RenderError::new("must be called with a single arg"))?;
        let string = string
            .value()
            .as_str()
            .ok_or_else(|| RenderError::new("argument must be a string"))?;

        let lines = string.split('\n');
        let mut lines: Vec<_> = lines.map(Json::from).collect();
        // Remove the last empty line if necessary.
        if let Some(Json::String(s)) = lines.last() {
            if s.is_empty() {
                lines.pop();
            }
        }

        Ok(ScopedJson::Derived(lines.into()))
    }
}

#[derive(Debug)]
struct RangeHelper;

impl RangeHelper {
    fn coerce_value(value: &Json) -> Option<i64> {
        value
            .as_i64()
            .or_else(|| value.as_f64().and_then(|val| to_i64(val.round())))
    }
}

impl HelperDef for RangeHelper {
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
        helper: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        const MESSAGE: &str = "`range` must be called with two integer args";

        let from = helper.param(0).ok_or_else(|| RenderError::new(MESSAGE))?;
        let from = Self::coerce_value(from.value()).ok_or_else(|| RenderError::new(MESSAGE))?;
        let to = helper.param(1).ok_or_else(|| RenderError::new(MESSAGE))?;
        let to = Self::coerce_value(to.value()).ok_or_else(|| RenderError::new(MESSAGE))?;

        let json: Vec<_> = (from..to).map(Json::from).collect();
        Ok(ScopedJson::Derived(json.into()))
    }
}

pub(super) fn register_helpers(reg: &mut Handlebars<'_>) {
    reg.register_helper("add", Box::new(OpsHelper::Add));
    reg.register_helper("sub", Box::new(OpsHelper::Sub));
    reg.register_helper("mul", Box::new(OpsHelper::Mul));
    reg.register_helper("div", Box::new(OpsHelper::Div));
    reg.register_helper("count_lines", Box::new(LineCounter));
    reg.register_helper("split_lines", Box::new(LineSplitter));
    reg.register_helper("range", Box::new(RangeHelper));
    reg.register_helper("scope", Box::new(ScopeHelper));
    reg.register_helper("eval", Box::new(EvalHelper));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_helper_basics() {
        let template = "{{#scope test_var=1}}Test var is: {{test_var}}{{/scope}}";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        let data = serde_json::json!({ "test": 3 });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered, "Test var is: 1");
    }

    #[test]
    fn reassigning_scope_vars() {
        let template = r#"
            {{#scope test_var="test"}}
                {{#test_var}}"{{test_var}} value"{{/test_var}}
                Test var is: {{test_var}}
            {{/scope}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        let data = serde_json::json!({ "test": 3 });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "Test var is: test value");
    }

    #[test]
    fn scope_helper_with_control_flow() {
        let template = r#"
            {{#scope result=""}}
                {{#each values}}
                    {{#if @first}}
                        {{result set=this}}
                    {{else}}
                        {{#result}}"{{result}}, {{this}}"{{/result}}
                    {{/if}}
                {{/each}}
                Concatenated: {{result}}
            {{/scope}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        let data = serde_json::json!({ "values": ["foo", "bar", "baz"] });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "Concatenated: foo, bar, baz");
    }

    #[test]
    fn add_helper_basics() {
        let template = "{{add 1 2 5}}";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("add", Box::new(OpsHelper::Add));
        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered, "8");
    }

    #[test]
    fn add_with_scope_var() {
        let template = "
            {{#scope lines=0 margins=0}}
                {{#each values}}
                    {{lines set=(add (lines) input.line_count output.line_count)}}
                    {{#if (eq output.line_count 0) }}
                        {{margins set=(add (margins) 1)}}
                    {{else}}
                        {{margins set=(add (margins) 2)}}
                    {{/if}}
                {{/each}}
                {{lines}}, {{margins}}
            {{/scope}}
        ";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));

        let data = serde_json::json!({
            "values": [{
                "input": { "line_count": 1 },
                "output": { "line_count": 2 },
            }, {
                "input": { "line_count": 2 },
                "output": { "line_count": 0 },
            }]
        });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "5, 3");
    }

    #[test]
    fn rounding_in_arithmetic_helpers() {
        let template = r#"
            {{div x y}}, {{div x y round=true}}, {{div x y round="down"}}, {{div x y round="up"}}
        "#;
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("div", Box::new(OpsHelper::Div));

        let data = serde_json::json!({ "x": 9, "y": 4 });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "2.25, 2, 2, 3");
    }

    #[test]
    fn eval_basics() {
        let template = r#"
            {{#*inline "define_constants"}}
            {
                {{! Bottom margin for each input or output block }}
                "BLOCK_MARGIN": 6,
                "USER_INPUT_PADDING": 10
            }
            {{/inline}}
            {{#with this as |$|}}
            {{#with (eval "define_constants") as |const|}}
            {{#with $}}
                {{margin}}: {{const.BLOCK_MARGIN}}px;
            {{/with}}
            {{/with}}
            {{/with}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("eval", Box::new(EvalHelper));
        let data = serde_json::json!({ "margin": "margin" });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "margin: 6px;");
    }

    #[test]
    fn eval_with_args() {
        let template = r#"
            {{#*inline "add_numbers"}}
                {{#scope sum=0}}
                    {{#each numbers}}
                        {{sum set=(add (sum) this)}}
                    {{/each}}
                    {{sum}}
                {{/scope}}
            {{/inline}}
            {{#with this as |$|}}
                {{#with (eval "add_numbers" numbers=$.num) as |sum|}}
                {{#with (eval "add_numbers" numbers=$.num) as |other_sum|}}
                    sum={{sum}}, other_sum={{other_sum}}
                {{/with}}
                {{/with}}
            {{/with}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("eval", Box::new(EvalHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));
        let data = serde_json::json!({ "num": [1, 2, 3, 4] });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "sum=10, other_sum=10");
    }

    #[test]
    fn line_counter() {
        let template = r#"
            {{count_lines text}}, {{count_lines text format="html"}}
        "#;
        let text = "test\ntest<br/>test";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("count_lines", Box::new(LineCounter));
        let data = serde_json::json!({ "text": text });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "2, 3");
    }

    #[test]
    fn line_splitter() {
        let template = "{{#each (split_lines text)}}{{this}}<br/>{{/each}}";
        let text = "test\nother test";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("split_lines", Box::new(LineSplitter));
        let data = serde_json::json!({ "text": text });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "test<br/>other test<br/>");

        let text = "test\nother test\n";
        let data = serde_json::json!({ "text": text });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "test<br/>other test<br/>");
    }

    #[test]
    fn range_helper_with_each_block() {
        let template = "{{#each (range 0 4)}}{{@index}}: {{lookup ../xs @index}}, {{/each}}";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("range", Box::new(RangeHelper));
        let data = serde_json::json!({ "xs": [2, 3, 5, 8] });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "0: 2, 1: 3, 2: 5, 3: 8,");
    }
}
