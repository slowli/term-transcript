//! Custom Handlebars helpers.

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

use handlebars::{
    BlockContext, Context, Handlebars, Helper, HelperDef, Output, PathAndJson, RenderContext,
    RenderError, RenderErrorReason, Renderable, ScopedJson, StringOutput,
};
use serde_json::Value as Json;
use unicode_width::UnicodeWidthStr;

/// Tries to convert an `f64` number to `i64` without precision loss.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn to_i64(value: f64) -> Option<i64> {
    const MAX_ACCURATE_VALUE: f64 = (1_i64 << 53) as f64;
    const MIN_ACCURATE_VALUE: f64 = -(1_i64 << 53) as f64;

    if value.fract() == 0.0 && (MIN_ACCURATE_VALUE..=MAX_ACCURATE_VALUE).contains(&value) {
        Some(value as i64)
    } else {
        None
    }
}

#[derive(Debug)]
struct PtrHelper;

impl PtrHelper {
    const NAME: &'static str = "ptr";
}

impl HelperDef for PtrHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", skip_all, err, fields(helper.params = ?helper.params()))
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _reg: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        _render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let value = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let value = value.value();

        let ptr = helper
            .param(1)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 1))?;
        let ptr = ptr.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "1".to_owned(),
                "string".to_owned(),
            )
        })?;

        let output = value.pointer(ptr).cloned().unwrap_or(Json::Null);
        Ok(ScopedJson::Derived(output))
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
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        const MESSAGE: &str = "`scope` must be called as block helper";

        let template = helper
            .template()
            .ok_or(RenderErrorReason::BlockContentRequired)?;
        if !helper.params().is_empty() {
            return Err(RenderErrorReason::Other(MESSAGE.to_owned()).into());
        }

        let mut pushed_block = false;
        if render_ctx.block().is_none() {
            render_ctx.push_block(BlockContext::default());
            pushed_block = true;
        }

        let current_block = render_ctx.block_mut().unwrap();
        let mut prev_vars = current_block.local_variables_mut().clone();
        for (&name, value) in helper.hash() {
            current_block.set_local_var(name, value.value().clone());
        }

        let scope_guard = ScopeGuard::new(helper.hash().keys().map(|&s| s.to_owned()).collect());
        let result = template.render(reg, ctx, render_ctx, out);

        // Reset the current block so that the added / modified block params are reset.
        if pushed_block {
            render_ctx.pop_block();
        } else {
            // Restore values in the current block to previous values.
            let current_block = render_ctx.block_mut().unwrap();
            let mut set_vars = scope_guard.set_vars();
            // Remove vars defined in this scope.
            for &name in helper.hash().keys() {
                set_vars.remove(name);
            }

            // Copy all changed vars.
            let local_vars = current_block.local_variables_mut();
            for var_name in set_vars {
                if prev_vars.get(&var_name).is_some() {
                    prev_vars.put(&var_name, local_vars.get(&var_name).unwrap().clone());
                }
            }
            *local_vars = prev_vars;
        }
        result
    }
}

#[derive(Debug)]
enum SetValue {
    Json(Json),
    Append(String),
}

impl SetValue {
    fn set(self, blocks: &mut VecDeque<BlockContext>, var_name: &str) -> Result<(), RenderError> {
        let var_parent = blocks
            .iter_mut()
            .find(|block| block.get_local_var(var_name).is_some())
            .ok_or_else(|| {
                RenderErrorReason::Other(format!("local var `{var_name}` is undefined"))
            })?;
        match self {
            Self::Json(value) => {
                var_parent.set_local_var(var_name, value);
            }
            Self::Append(s) => {
                let prev_value = var_parent.get_local_var(var_name).unwrap();
                let prev_value = prev_value.as_str().ok_or_else(|| {
                    RenderErrorReason::Other(format!(
                        "cannot append to a non-string local var `{var_name}`"
                    ))
                })?;

                let mut new_value = prev_value.to_owned();
                new_value.push_str(&s);
                var_parent.set_local_var(var_name, new_value.into());
            }
        }
        Ok(())
    }
}

thread_local! {
    static SET_VARS: RefCell<HashSet<String>> = RefCell::default();
}
thread_local! {
    static DEFINED_VARS: RefCell<HashSet<String>> = RefCell::default();
}

#[must_use]
#[derive(Debug)]
struct ScopeGuard {
    set_vars: HashSet<String>,
    defined_vars: HashSet<String>,
}

impl ScopeGuard {
    fn new(defined_vars: HashSet<String>) -> Self {
        Self {
            set_vars: SET_VARS.take(),
            defined_vars: DEFINED_VARS.replace(defined_vars),
        }
    }

    fn set_vars(self) -> HashSet<String> {
        DEFINED_VARS.set(self.defined_vars);
        SET_VARS.replace(self.set_vars)
    }
}

#[derive(Debug)]
struct SetHelper;

impl SetHelper {
    const NAME: &'static str = "set";

    fn call_as_block<'reg: 'rc, 'rc>(
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<(), RenderError> {
        let var_name = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(SetHelper::NAME, 0))?;
        let var_name: &'rc str = var_name
            .try_get_constant_value()
            .and_then(Json::as_str)
            .ok_or_else(|| {
                RenderErrorReason::ParamTypeMismatchForName(
                    SetHelper::NAME,
                    "0".to_owned(),
                    "constant string".to_owned(),
                )
            })?;

        let is_append = helper
            .hash_get("append")
            .is_some_and(|val| val.value().as_bool() == Some(true));
        let value = if let Some(template) = helper.template() {
            let mut output = StringOutput::new();
            template.render(reg, ctx, render_ctx, &mut output)?;
            let raw_string = output.into_string()?;
            if is_append {
                SetValue::Append(raw_string)
            } else {
                let json = serde_json::from_str(&raw_string).map_err(RenderErrorReason::from)?;
                SetValue::Json(json)
            }
        } else {
            SetValue::Json(Json::Null)
        };

        let mut blocks = render_ctx.replace_blocks(VecDeque::default());
        let result = value.set(&mut blocks, var_name);
        render_ctx.replace_blocks(blocks);
        if result.is_ok() {
            SET_VARS.with_borrow_mut(|vars| vars.insert(var_name.to_owned()));
        }
        result
    }

    fn batch_set<'a, 'rc: 'a>(
        blocks: &mut VecDeque<BlockContext>,
        values: impl Iterator<Item = (&'a str, &'a PathAndJson<'rc>)>,
    ) -> Result<(), RenderError> {
        for (name, value) in values {
            let var_parent = blocks
                .iter_mut()
                .find(|block| block.get_local_var(name).is_some())
                .ok_or_else(|| {
                    RenderErrorReason::Other(format!("local var `{name}` is undefined"))
                })?;
            var_parent.set_local_var(name, value.value().clone());
            SET_VARS.with_borrow_mut(|vars| vars.insert(name.to_owned()));
        }
        Ok(())
    }
}

impl HelperDef for SetHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(
                self = ?self,
                helper.is_block = helper.is_block(),
                helper.var = ?helper.param(0),
                helper.hash = ?helper.hash(),
            )
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        if helper.is_block() {
            Self::call_as_block(helper, reg, ctx, render_ctx)?;
            return Ok(ScopedJson::Constant(&Json::Null));
        }

        let values = helper.hash().iter().map(|(name, value)| (*name, value));
        let mut blocks = render_ctx.replace_blocks(VecDeque::default());
        let result = Self::batch_set(&mut blocks, values);
        render_ctx.replace_blocks(blocks);
        result?;

        Ok(ScopedJson::Constant(&Json::Null))
    }
}

#[derive(Debug)]
enum OpsHelper {
    Add,
    Mul,
    Sub,
    Div,
    Min,
    Max,
}

impl OpsHelper {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Sub => "sub",
            Self::Div => "div",
            Self::Min => "min",
            Self::Max => "max",
        }
    }

    fn accumulate_i64(&self, mut values: impl Iterator<Item = i64>) -> i64 {
        match self {
            Self::Add => values.sum(),
            Self::Mul => values.product(),
            // `unwrap`s are safe because of previous checks
            Self::Sub => values.next().unwrap() - values.next().unwrap(),
            Self::Min => values.min().unwrap(),
            Self::Max => values.max().unwrap(),
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
            Self::Min => values.reduce(f64::min).unwrap(),
            Self::Max => values.reduce(f64::max).unwrap(),
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
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        if matches!(self, Self::Sub | Self::Div) && helper.params().len() != 2 {
            let message = format!("`{}` expects exactly 2 number args", self.as_str());
            return Err(RenderErrorReason::Other(message).into());
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
            if let Some(rounding) = helper.hash_get("round") {
                if matches!(rounding.value(), Json::Bool(true)) {
                    acc = acc.round();
                } else if rounding.value().as_str() == Some("up") {
                    acc = acc.ceil();
                } else if rounding.value().as_str() == Some("down") {
                    acc = acc.floor();
                }
            }
            // Try to present the value as `i64` (this could be beneficial for other helpers).
            // If this doesn't work, present it as an original floating-point value.
            let acc: Json = to_i64(acc).map_or_else(|| acc.into(), Into::into);
            Ok(ScopedJson::Derived(acc))
        } else {
            let message = "all args must be numbers";
            Err(RenderErrorReason::Other(message.to_owned()).into())
        }
    }
}

/// Splats local vars (without the '@' prefix) into the provided object.
#[derive(Debug)]
struct SplatVarsHelper;

impl SplatVarsHelper {
    const NAME: &'static str = "splat_vars";
}

impl HelperDef for SplatVarsHelper {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "trace",
            skip_all, err,
            fields(
                self = ?self,
                helper.base = ?helper.param(0),
            )
        )
    )]
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _reg: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let base = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let base = base.value().as_object().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "object".to_owned(),
            )
        })?;
        let mut merged = base.clone();

        if let Some(current_block) = render_ctx.block() {
            DEFINED_VARS.with_borrow(|var_names| {
                for name in var_names {
                    let value = current_block.get_local_var(name).unwrap();
                    merged.insert(name.clone(), value.clone());
                }
            });
        }

        Ok(ScopedJson::Derived(merged.into()))
    }
}

#[derive(Debug)]
struct EvalHelper;

impl EvalHelper {
    const NAME: &'static str = "eval";
}

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
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let partial_name = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let mut partial_name = partial_name.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

        let mut is_raw = false;
        if let Some(name) = partial_name.strip_prefix(">") {
            is_raw = true;
            partial_name = name;
        }

        let partial = render_ctx
            .get_partial(partial_name)
            .ok_or_else(|| RenderErrorReason::PartialNotFound(partial_name.to_owned()))?;

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
        let output = output.into_string()?;
        let json: Json = if is_raw {
            output.into()
        } else {
            serde_json::from_str(&output).map_err(RenderErrorReason::from)?
        };
        Ok(ScopedJson::Derived(json))
    }
}

#[derive(Debug)]
struct LineCounter;

impl LineCounter {
    const NAME: &'static str = "count_lines";
}

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
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let string = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let string = string.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

        let mut lines = bytecount::count(string.as_bytes(), b'\n');
        if !string.is_empty() && !string.ends_with('\n') {
            lines += 1;
        }

        let lines = u64::try_from(lines)
            .map_err(|err| RenderErrorReason::Other(format!("cannot convert length: {err}")))?;
        Ok(ScopedJson::Derived(lines.into()))
    }
}

#[derive(Debug)]
struct LineSplitter;

impl LineSplitter {
    const NAME: &'static str = "split_lines";
}

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
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let string = helper
            .param(0)
            .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let string = string.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

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
    const NAME: &'static str = "range";

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
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let from = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let from = Self::coerce_value(from.value()).ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "integer".to_owned(),
            )
        })?;
        let to = helper
            .param(1)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 1))?;
        let to = Self::coerce_value(to.value()).ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "1".to_owned(),
                "integer".to_owned(),
            )
        })?;

        let json: Vec<_> = (from..to).map(Json::from).collect();
        Ok(ScopedJson::Derived(json.into()))
    }
}

#[derive(Debug)]
struct RepeatHelper;

impl RepeatHelper {
    const NAME: &'static str = "repeat";
}

impl HelperDef for RepeatHelper {
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
        let repeated_str = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 0))?;
        let repeated_str = repeated_str.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

        let quantity = helper
            .param(1)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(Self::NAME, 1))?;
        let quantity = quantity.value().as_u64().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "integer".to_owned(),
            )
        })?;

        let quantity = quantity
            .try_into()
            .map_err(|_| RenderErrorReason::Other("quantity is too large".to_owned()))?;
        let output = repeated_str.repeat(quantity);
        Ok(ScopedJson::Derived(output.into()))
    }
}

#[derive(Debug)]
struct RoundHelper;

impl RoundHelper {
    const NAME: &'static str = "round";
    const MAX_DIGITS: u64 = 15;
}

impl HelperDef for RoundHelper {
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
        let val = val.value().as_f64().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "float".to_owned(),
            )
        })?;

        let digits = if let Some(digits) = helper.hash_get("digits") {
            digits.value().as_u64().ok_or_else(|| {
                RenderErrorReason::ParamTypeMismatchForName(
                    Self::NAME,
                    "digits".to_owned(),
                    "non-negative int".to_owned(),
                )
            })?
        } else {
            0
        };
        if digits > Self::MAX_DIGITS {
            let msg = format!("too many digits: {digits}, use <= {}", Self::MAX_DIGITS);
            return Err(RenderErrorReason::Other(msg).into());
        }

        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        // ^ Partially guarded by checks; the remaining precision loss is OK
        let rounded: Json = {
            let pow10 = 10.0_f64.powi(digits.try_into().unwrap());
            let rounded = (val * pow10).round() / pow10;
            to_i64(rounded).map_or_else(|| rounded.into(), Into::into)
        };
        Ok(ScopedJson::Derived(rounded))
    }
}

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

#[derive(Debug)]
struct TrimHelper;

impl TrimHelper {
    const NAME: &'static str = "trim";
}

impl HelperDef for TrimHelper {
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
        let val = val.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

        let side = helper
            .hash_get("side")
            .map(|val| {
                val.value().as_str().ok_or_else(|| {
                    RenderErrorReason::ParamTypeMismatchForName(
                        Self::NAME,
                        "side".to_owned(),
                        "one of `start`, `end`, or `both`".to_owned(),
                    )
                })
            })
            .transpose()?;

        let trimmed = match side {
            None | Some("both") => val.trim(),
            Some("start") => val.trim_start(),
            Some("end") => val.trim_end(),
            _ => {
                let err = RenderErrorReason::ParamTypeMismatchForName(
                    Self::NAME,
                    "side".to_owned(),
                    "one of `start`, `end`, or `both`".to_owned(),
                );
                return Err(err.into());
            }
        };
        Ok(ScopedJson::Derived(trimmed.into()))
    }
}

#[derive(Debug)]
struct CharWidthHelper;

impl CharWidthHelper {
    const NAME: &'static str = "char_width";
}

impl HelperDef for CharWidthHelper {
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
        let val = val.value().as_str().ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                Self::NAME,
                "0".to_owned(),
                "string".to_owned(),
            )
        })?;

        let width = val.width();
        Ok(ScopedJson::Derived(width.into()))
    }
}

pub(super) fn register_helpers(reg: &mut Handlebars<'_>) {
    reg.register_helper("add", Box::new(OpsHelper::Add));
    reg.register_helper("sub", Box::new(OpsHelper::Sub));
    reg.register_helper("mul", Box::new(OpsHelper::Mul));
    reg.register_helper("div", Box::new(OpsHelper::Div));
    reg.register_helper("min", Box::new(OpsHelper::Min));
    reg.register_helper("max", Box::new(OpsHelper::Max));

    reg.register_helper(PtrHelper::NAME, Box::new(PtrHelper));
    reg.register_helper(RoundHelper::NAME, Box::new(RoundHelper));
    reg.register_helper(LineCounter::NAME, Box::new(LineCounter));
    reg.register_helper(LineSplitter::NAME, Box::new(LineSplitter));
    reg.register_helper(RangeHelper::NAME, Box::new(RangeHelper));
    reg.register_helper("scope", Box::new(ScopeHelper));
    reg.register_helper(SetHelper::NAME, Box::new(SetHelper));
    reg.register_helper(SplatVarsHelper::NAME, Box::new(SplatVarsHelper));
    reg.register_helper(EvalHelper::NAME, Box::new(EvalHelper));
    reg.register_helper(RepeatHelper::NAME, Box::new(RepeatHelper));
    reg.register_helper(TypeofHelper::NAME, Box::new(TypeofHelper));
    reg.register_helper(TrimHelper::NAME, Box::new(TrimHelper));
    reg.register_helper(CharWidthHelper::NAME, Box::new(CharWidthHelper));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptr_helper_basics() {
        let template =
            r#"{{ptr this "/test/str"}}, {{ptr this "/test/missing"}}, {{ptr this "/array/0"}}"#;
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("ptr", Box::new(PtrHelper));
        let data = serde_json::json!({
            "test": { "str": "!" },
            "array": [2, 3],
        });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered, "!, , 2");
    }

    #[test]
    fn scope_helper_basics() {
        let template = "{{#scope test_var=1}}Test var is: {{@test_var}}{{/scope}}";
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
                {{#set "test_var"}}"{{@test_var}} value"{{/set}}
                Test var is: {{@test_var}}
            {{/scope}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        let data = serde_json::json!({ "test": 3 });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "Test var is: test value");
    }

    #[test]
    fn reassigning_scope_vars_via_appending() {
        let template = r#"
            {{#scope test_var="test"}}
                {{#set "test_var" append=true}} value{{/set}}
                {{#set "test_var" append=true}}!{{/set}}
                Test var is: {{@test_var}}
            {{/scope}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        let data = serde_json::json!({ "test": 3 });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "Test var is: test value!");
    }

    #[test]
    fn scope_helper_with_control_flow() {
        let template = r#"
            {{#scope result=""}}
                {{#each values}}
                    {{#if @first}}
                        {{set result=this}}
                    {{else}}
                        {{#set "result"}}"{{@../result}}, {{this}}"{{/set}}
                    {{/if}}
                {{/each}}
                Concatenated: {{@result}}
            {{/scope}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
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
    fn min_helper_basics() {
        let template = "{{min 2 -1 5}}";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("min", Box::new(OpsHelper::Min));
        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered, "-1");
    }

    #[test]
    fn add_with_scope_var() {
        let template = "
            {{#scope lines=0 margins=0}}
                {{#each values}}
                    {{set lines=(add @../lines input.line_count output.line_count)}}
                    {{#if (eq output.line_count 0) }}
                        {{set margins=(add @../margins 1)}}
                    {{else}}
                        {{set margins=(add @../margins 2)}}
                    {{/if}}
                {{/each}}
                {{@lines}}, {{@margins}}
            {{/scope}}
        ";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
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
    fn rounding_helper() {
        let template = "
            {{round 10.5}}, {{round 10.5 digits=2}}, {{round (mul 14 (div 1050 1000)) digits=2}}
        ";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("mul", Box::new(OpsHelper::Mul));
        handlebars.register_helper("div", Box::new(OpsHelper::Div));
        handlebars.register_helper("round", Box::new(RoundHelper));
        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered.trim(), "11, 10.5, 14.7");
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
                        {{set sum=(add @../sum this)}}
                    {{/each}}
                    {{@sum}}
                {{/scope}}
            {{/inline}}
            {{#with (eval "add_numbers" numbers=@root.num) as |sum|}}
            {{#with (eval "add_numbers" numbers=@root.num) as |other_sum|}}
                sum={{sum}}, other_sum={{other_sum}}
            {{/with}}
            {{/with}}
        "#;

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        handlebars.register_helper("eval", Box::new(EvalHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));
        let data = serde_json::json!({ "num": [1, 2, 3, 4] });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "sum=10, other_sum=10");
    }

    #[test]
    fn line_counter() {
        let template = "{{count_lines text}}";
        let text = "test\ntest test";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("count_lines", Box::new(LineCounter));
        let data = serde_json::json!({ "text": text });
        let rendered = handlebars.render_template(template, &data).unwrap();
        assert_eq!(rendered.trim(), "2");
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

    #[test]
    fn repeat_helper_basics() {
        let template = "{{repeat \"█\" 5}}";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("repeat", Box::new(RepeatHelper));

        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered.trim(), "█████");
    }

    #[test]
    fn set_helper() {
        let template = "{{#scope test_var=1}}{{set test_var=(add @test_var 1)}}Test var: {{@test_var}}{{/scope}}";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));

        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered, "Test var: 2");
    }

    #[test]
    fn set_helper_as_block() {
        let template = r#"{{#scope test_var=1 greet="Hello"~}}
            {{~#set "test_var"}}{{add @test_var 1}}{{/set~}}
            {{~#set "greet" append=true}}, world!{{/set~}}
            {{@greet}} {{@test_var}}
        {{~/scope}}"#;
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));

        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered, "Hello, world! 2");
    }

    #[test]
    fn set_helper_with_scope() {
        let template = "
            {{~#scope test_var=1~}}
              {{~#each [1, 2, 3] as |num|~}}
                {{~set test_var=(add @../test_var num)}}-{{@../test_var~}}
              {{~/each~}}
            {{~/scope~}}
        ";
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));
        handlebars.register_helper("add", Box::new(OpsHelper::Add));

        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(rendered.trim(), "-2-4-7");
    }

    #[test]
    fn embedded_scopes() {
        let template = r"
            {{~#scope x=1 z=100~}}
                x={{@x}},
                {{~#scope x=2 y=3~}}
                  x={{@x}},y={{@y}},
                  {{~set x=4 y=5~}}
                  x={{@x}},y={{@y}},z={{@z}},
                  {{~set z=-100~}}
                  z={{@z}},
                {{~/scope~}}
                x={{@x}},z={{@z}}
            {{~/scope~}}
        ";

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars.register_helper("scope", Box::new(ScopeHelper));
        handlebars.register_helper("set", Box::new(SetHelper));

        let rendered = handlebars.render_template(template, &()).unwrap();
        assert_eq!(
            rendered.trim(),
            "x=1,x=2,y=3,x=4,y=5,z=100,z=-100,x=1,z=-100"
        );
    }
}
