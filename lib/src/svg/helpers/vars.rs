//! Helpers managing local variables.

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

use handlebars::{
    BlockContext, Context, Handlebars, Helper, HelperDef, Output, RenderContext, RenderError,
    RenderErrorReason, Renderable, ScopedJson, StringOutput,
};
use serde_json::Value as Json;

#[derive(Debug)]
pub(super) struct ScopeHelper;

impl HelperDef for ScopeHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        const MESSAGE: &str = "`scope` must be called as block helper";

        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

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
    fn set(self, var_parent: &mut BlockContext, var_name: &str) -> Result<(), RenderError> {
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
pub(super) struct SetHelper;

impl SetHelper {
    pub(super) const NAME: &'static str = "set";

    fn call_as_block<'reg: 'rc, 'rc>(
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
        var_name: &str,
        block_depth: usize,
    ) -> Result<(), RenderError> {
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
        let result = Self::get_parent(&mut blocks, var_name, block_depth)
            .and_then(|var_parent| value.set(var_parent, var_name));
        render_ctx.replace_blocks(blocks);
        if result.is_ok() {
            SET_VARS.with_borrow_mut(|vars| vars.insert(var_name.to_owned()));
        }
        result
    }

    fn get_parent<'block, 'rc>(
        blocks: &'block mut VecDeque<BlockContext<'rc>>,
        var_name: &str,
        block_depth: usize,
    ) -> Result<&'block mut BlockContext<'rc>, RenderError> {
        blocks
            .get_mut(block_depth)
            .filter(|block| block.get_local_var(var_name).is_some())
            .ok_or_else(|| {
                RenderErrorReason::Other(format!("local var `{var_name}` is undefined")).into()
            })
    }

    fn set_var(
        helper: &Helper<'_>,
        blocks: &mut VecDeque<BlockContext>,
        var_name: &str,
        block_depth: usize,
    ) -> Result<(), RenderError> {
        let new_value = helper
            .param(1)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(SetHelper::NAME, 1))?;

        let var_parent = Self::get_parent(blocks, var_name, block_depth)?;
        var_parent.set_local_var(var_name, new_value.value().clone());
        SET_VARS.with_borrow_mut(|vars| vars.insert(var_name.to_owned()));
        Ok(())
    }
}

impl HelperDef for SetHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        reg: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

        let var = helper
            .param(0)
            .ok_or(RenderErrorReason::ParamNotFoundForIndex(SetHelper::NAME, 0))?;
        let mut var_name = var
            .relative_path()
            .and_then(|name| name.strip_prefix('@'))
            .ok_or_else(|| {
                RenderErrorReason::ParamTypeMismatchForName(
                    SetHelper::NAME,
                    "0".to_owned(),
                    "path to local variable".to_owned(),
                )
            })?;
        if var.is_value_missing() {
            let err = RenderErrorReason::MissingVariable(Some(format!("@{var_name}")));
            return Err(err.into());
        }

        let mut block_depth = 0;
        while let Some(tail) = var_name.strip_prefix("../") {
            var_name = tail;
            block_depth += 1;
        }

        if helper.is_block() {
            Self::call_as_block(helper, reg, ctx, render_ctx, var_name, block_depth)?;
        } else {
            let mut blocks = render_ctx.replace_blocks(VecDeque::default());
            let result = Self::set_var(helper, &mut blocks, var_name, block_depth);
            render_ctx.replace_blocks(blocks);
            result?;
        }

        Ok(ScopedJson::Constant(&Json::Null))
    }
}

/// Splats local vars (without the '@' prefix) into the provided object.
#[derive(Debug)]
pub(super) struct SplatVarsHelper;

impl SplatVarsHelper {
    pub(super) const NAME: &'static str = "splat_vars";
}

impl HelperDef for SplatVarsHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _reg: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        render_ctx: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        #[cfg(feature = "tracing")]
        let _entered_span = helper_span!(helper);

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
