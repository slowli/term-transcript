//! Arithmetic helpers.

use handlebars::{
    Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, RenderErrorReason,
    ScopedJson,
};
use serde_json::Value as Json;

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
pub(super) enum OpsHelper {
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
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

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
            let acc = self.accumulate_f64(values);
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

#[derive(Debug)]
pub(super) struct RangeHelper;

impl RangeHelper {
    pub(super) const NAME: &'static str = "range";

    fn coerce_value(value: &Json) -> Option<i64> {
        value
            .as_i64()
            .or_else(|| value.as_f64().and_then(|val| to_i64(val.round())))
    }
}

impl HelperDef for RangeHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

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

#[derive(Debug, Clone, Copy, Default)]
enum RoundingMode {
    Up,
    Down,
    #[default]
    Nearest,
}

impl RoundingMode {
    const EXPECTED: &'static str = "one of 'up' / 'ceil', 'down' / 'floor', or 'nearest' / 'round'";

    fn new(raw: &str) -> Result<Self, String> {
        Ok(match raw {
            "up" | "ceil" => Self::Up,
            "down" | "floor" => Self::Down,
            "nearest" | "round" => Self::Nearest,
            _ => {
                return Err(format!(
                    "Unknown rounding mode: {raw}; expected {exp}",
                    exp = Self::EXPECTED
                ))
            }
        })
    }

    fn apply(self, val: f64) -> f64 {
        match self {
            Self::Up => val.ceil(),
            Self::Down => val.floor(),
            Self::Nearest => val.round(),
        }
    }
}

#[derive(Debug)]
pub(super) struct RoundHelper;

impl RoundHelper {
    pub(super) const NAME: &'static str = "round";
    const MAX_DIGITS: u64 = 15;
}

impl HelperDef for RoundHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

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

        let mode = if let Some(mode) = helper.hash_get("mode") {
            let mode = mode.value().as_str().ok_or_else(|| {
                RenderErrorReason::ParamTypeMismatchForName(
                    Self::NAME,
                    "mode".to_owned(),
                    RoundingMode::EXPECTED.to_owned(),
                )
            })?;
            RoundingMode::new(mode).map_err(|err| {
                RenderErrorReason::ParamTypeMismatchForName(Self::NAME, "mode".to_owned(), err)
            })?
        } else {
            RoundingMode::default()
        };

        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        // ^ Partially guarded by checks; the remaining precision loss is OK
        let rounded: Json = {
            let pow10 = 10.0_f64.powi(digits.try_into().unwrap());
            let rounded = mode.apply(val * pow10) / pow10;
            to_i64(rounded).map_or_else(|| rounded.into(), Into::into)
        };
        Ok(ScopedJson::Derived(rounded))
    }
}
