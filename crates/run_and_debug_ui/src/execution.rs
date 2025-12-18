use anyhow::{anyhow, Result};
use run_and_debug::recipe::{ExecutionMode, Recipe};
use task::{DebugScenario, ResolvedTask, TaskContext};

pub enum PreparedExecution {
    Task(ResolvedTask),
    Debug(DebugScenario),
}

pub fn prepare_execution(
    mode_name: &str,
    recipe: &Recipe,
    task_context: &TaskContext,
) -> Result<PreparedExecution> {

    let mode = recipe
        .get_mode(mode_name)
        .ok_or(anyhow!("Mode '{}' not found in recipe '{}'", mode_name, recipe.name))?;

    match mode {
        ExecutionMode::Task(task_template) => {
            let resolved_task = task_template
                .resolve_task(&recipe.name, task_context)
                .ok_or(anyhow!("Failed to resolve task"))?;

            Ok(PreparedExecution::Task(resolved_task))
        }
        ExecutionMode::Debug(debug_scenario) => {
            Ok(PreparedExecution::Debug(debug_scenario.clone()))
        }
    }
}
