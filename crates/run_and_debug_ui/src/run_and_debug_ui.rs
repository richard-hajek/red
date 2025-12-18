
pub mod execution;
pub mod toolbar;

use gpui::{actions, App, Global, UpdateGlobal};
use project::RunAndDebugSourceKind;
use std::sync::Arc;
use run_and_debug::configuration::RunnableConfiguration;

// actions!(
//     configurations,
//     [
//         RunConfiguration,
//         DebugConfiguration,
//         SelectConfiguration
//     ]
// );

#[derive(Default, Clone)]
pub struct SelectedConfiguration {
    pub selection: Option<(RunAndDebugSourceKind, Arc<RunnableConfiguration>)>,
}

impl Global for SelectedConfiguration {}

pub fn init(cx: &mut App) {
    cx.set_global(SelectedConfiguration::default());
}

pub fn set_selected_configuration(
    source: RunAndDebugSourceKind,
    template: RunnableConfiguration,
    cx: &mut App,
) {
    log::info!("Configuration selected: '{}'", template.label);
    SelectedConfiguration::update_global(cx, |state, _cx| {
        state.selection = Some((source, Arc::new(template)));
    });
}
