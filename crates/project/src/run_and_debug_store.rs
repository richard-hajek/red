use std::{path::Path, sync::Arc};

use gpui::{App, Context, Entity, EventEmitter, WeakEntity};
use language::LanguageToolchainStore;
use rpc::AnyProtoClient;
use settings::{InvalidSettingsError, SettingsLocation};

use crate::{
    RunAndDebugInventory, ProjectEnvironment,
    task_store::TaskStore, worktree_store::WorktreeStore,
};

pub enum RunAndDebugStore {
    Functional(StoreState),
    Noop,
}

pub struct StoreState {
    run_and_debug_inventory: Entity<RunAndDebugInventory>,
    _worktree_store: Entity<WorktreeStore>,
    _toolchain_store: Arc<dyn LanguageToolchainStore>,
    _task_store: WeakEntity<TaskStore>,
    _environment: Entity<ProjectEnvironment>,
}

impl EventEmitter<crate::Event> for RunAndDebugStore {}

#[derive(Debug)]
pub enum RunAndDebugSettingsLocation<'a> {
    Global(&'a Path),
    Worktree(SettingsLocation<'a>),
}

impl RunAndDebugStore {
    pub fn local(
        worktree_store: Entity<WorktreeStore>,
        toolchain_store: std::sync::Arc<dyn LanguageToolchainStore>,
        task_store: WeakEntity<TaskStore>,
        environment: Entity<ProjectEnvironment>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::Functional(StoreState {
            run_and_debug_inventory: RunAndDebugInventory::new(cx),
            _toolchain_store: toolchain_store,
            _worktree_store: worktree_store,
            _task_store: task_store,
            _environment: environment,
        })
    }

    /// Remote mode is not supported for run and debug - returns a Noop store
    pub fn remote(
        _worktree_store: Entity<WorktreeStore>,
        _toolchain_store: std::sync::Arc<dyn LanguageToolchainStore>,
        _task_store: WeakEntity<TaskStore>,
        _upstream_client: AnyProtoClient,
        _project_id: u64,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self::Noop
    }

    pub fn run_and_debug_inventory(&self) -> Option<&Entity<RunAndDebugInventory>> {
        match self {
            RunAndDebugStore::Functional(state) => Some(&state.run_and_debug_inventory),
            RunAndDebugStore::Noop => None,
        }
    }

    /// Get a recipe by name from the inventory
    pub fn get_recipe(&self, name: &str, worktree: Option<worktree::WorktreeId>, cx: &App) -> Option<run_and_debug::configuration::Recipe> {
        let inventory = self.run_and_debug_inventory()?;
        inventory.read(cx).get_recipe(name, worktree)
    }

    pub(super) fn update_run_and_debug_store(
        &self,
        location: RunAndDebugSettingsLocation<'_>,
        raw_configurations_json: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidSettingsError> {
        log::info!("update_user_configurations called with location: {location:?}");
        let configuration_inventory = match self {
            RunAndDebugStore::Functional(state) => &state.run_and_debug_inventory,
            RunAndDebugStore::Noop => {
                log::warn!("ConfigurationStore is Noop, skipping update");
                return Ok(());
            }
        };
        let raw_configurations_json = raw_configurations_json
            .map(|json| json.trim())
            .filter(|json| !json.is_empty());
        
        if let Some(json) = raw_configurations_json {
            log::info!("=== RAW CONFIGURATION JSON ({} bytes) ===", json.len());
            log::info!("{}", json);
        } else {
            log::info!("Received empty/null configuration, clearing configurations");
        }

        configuration_inventory.update(cx, |inventory, _| {
            use crate::run_and_debug_inventory::{
                SettingsLocation as InvLocation, parse_configuration_file,
            };
            
            let parsed_configurations = if let Some(json_str) = raw_configurations_json {
                log::info!("Parsing configuration JSON...");
                let configs = parse_configuration_file(json_str.to_string())?;
                log::info!("Successfully parsed {} configurations", configs.len());
                for (i, config) in configs.iter().enumerate() {
                    log::info!("  Config {}: label='{}', recipe='{}'", i + 1, config.label, config.recipe);
                }
                configs
            } else {
                log::info!("No JSON content, clearing configurations");
                Vec::new()
            };

            let inv_location = match location {
                RunAndDebugSettingsLocation::Global(path) => {
                    log::info!("Updating global configurations at {:?}", path);
                    InvLocation::Global(path.to_path_buf())
                }
                RunAndDebugSettingsLocation::Worktree(settings_location) => {
                    log::info!("Updating worktree configurations: worktree_id={:?}, path={:?}", 
                              settings_location.worktree_id, settings_location.path);
                    InvLocation::Worktree {
                        worktree_id: settings_location.worktree_id,
                        directory_in_worktree: Arc::from(settings_location.path.as_ref()),
                    }
                }
            };

            inventory.update_configurations(inv_location, parsed_configurations);
            log::info!("Configuration inventory updated successfully");
            Ok(())
        })?;
        
        // Explicitly notify observers that the inventory has changed
        configuration_inventory.update(cx, |_, cx| {
            cx.notify();
        });
        
        Ok(())
    }

    pub(super) fn update_recipes(
        &self,
        location: RunAndDebugSettingsLocation<'_>,
        raw_recipes_json: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidSettingsError> {
        log::info!("update_recipes called with location: {location:?}");
        let inventory = match self {
            RunAndDebugStore::Functional(state) => &state.run_and_debug_inventory,
            RunAndDebugStore::Noop => {
                log::warn!("RunAndDebugStore is Noop, skipping update");
                return Ok(());
            }
        };
        let raw_recipes_json = raw_recipes_json
            .map(|json| json.trim())
            .filter(|json| !json.is_empty());
        
        if let Some(json) = raw_recipes_json {
            log::info!("=== RAW RECIPES JSON ({} bytes) ===", json.len());
            log::info!("{}", json);
        } else {
            log::info!("Received empty/null recipes, clearing recipes");
        }

        inventory.update(cx, |inventory, _| {
            use crate::run_and_debug_inventory::{
                SettingsLocation as InvLocation, parse_recipe_file,
            };
            
            let parsed_recipes = if let Some(json_str) = raw_recipes_json {
                log::info!("Parsing recipes JSON...");
                let recipes = parse_recipe_file(json_str.to_string())?;
                log::info!("Successfully parsed {} recipes", recipes.len());
                for (i, recipe) in recipes.iter().enumerate() {
                    log::info!("  Recipe {}: name='{}'", i + 1, recipe.name);
                }
                recipes
            } else {
                log::info!("No JSON content, clearing recipes");
                Vec::new()
            };

            let inv_location = match location {
                RunAndDebugSettingsLocation::Global(path) => {
                    log::info!("Updating global recipes at {:?}", path);
                    InvLocation::Global(path.to_path_buf())
                }
                RunAndDebugSettingsLocation::Worktree(settings_location) => {
                    log::info!("Updating worktree recipes: worktree_id={:?}, path={:?}", 
                              settings_location.worktree_id, settings_location.path);
                    InvLocation::Worktree {
                        worktree_id: settings_location.worktree_id,
                        directory_in_worktree: Arc::from(settings_location.path.as_ref()),
                    }
                }
            };

            inventory.update_recipes(inv_location, parsed_recipes);
            log::info!("Recipe inventory updated successfully");
            Ok(())
        })?;
        
        // Explicitly notify observers that the inventory has changed
        inventory.update(cx, |_, cx| {
            cx.notify();
        });
        
        Ok(())
    }
}
