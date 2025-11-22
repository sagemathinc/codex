use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::exec::ExecToolCallOutput;
use crate::exec::StdoutStream;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::execute_env;
use crate::tools::runtimes::apply_patch::ApplyPatchRequest;
use crate::tools::runtimes::build_command_spec;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::CODEX_APPLY_PATCH_ARG1;

pub type DynToolExecutor = Arc<dyn ToolExecutor>;

pub(crate) fn default_tool_executor() -> DynToolExecutor {
    Arc::new(DefaultToolExecutor::default())
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn run_shell(
        &self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError>;

    async fn run_apply_patch(
        &self,
        req: &ApplyPatchRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError>;
}

#[derive(Default)]
struct DefaultToolExecutor;

#[async_trait]
impl ToolExecutor for DefaultToolExecutor {
    async fn run_shell(
        &self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = build_command_spec(
            &req.command,
            &req.cwd,
            &req.env,
            req.timeout_ms.into(),
            req.with_escalated_permissions,
            req.justification.clone(),
        )?;
        let env = attempt
            .env_for(spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }

    async fn run_apply_patch(
        &self,
        req: &ApplyPatchRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = build_apply_patch_spec(req)?;
        let env = attempt
            .env_for(spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<StdoutStream> {
    Some(StdoutStream {
        sub_id: ctx.turn.sub_id.clone(),
        call_id: ctx.call_id.clone(),
        tx_event: ctx.session.get_tx_event(),
    })
}

fn build_apply_patch_spec(req: &ApplyPatchRequest) -> Result<CommandSpec, ToolError> {
    use std::env;
    let exe = if let Some(path) = &req.codex_exe {
        path.clone()
    } else {
        env::current_exe()
            .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))?
    };
    let program = exe.to_string_lossy().to_string();
    Ok(CommandSpec {
        program,
        args: vec![CODEX_APPLY_PATCH_ARG1.to_string(), req.patch.clone()],
        cwd: req.cwd.clone(),
        expiration: req.timeout_ms.into(),
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
    })
}
