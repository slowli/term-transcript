//! String helpers.

use handlebars::{
    Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, RenderErrorReason,
    ScopedJson,
};
use serde_json::Value as Json;
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
pub(super) struct LineCounter;

impl LineCounter {
    pub(super) const NAME: &'static str = "count_lines";
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
pub(super) struct LineSplitter;

impl LineSplitter {
    pub(super) const NAME: &'static str = "split_lines";
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
pub(super) struct RepeatHelper;

impl RepeatHelper {
    pub(super) const NAME: &'static str = "repeat";
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
pub(super) struct TrimHelper;

impl TrimHelper {
    pub(super) const NAME: &'static str = "trim";
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
pub(super) struct CharWidthHelper;

impl CharWidthHelper {
    pub(super) const NAME: &'static str = "char_width";
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
