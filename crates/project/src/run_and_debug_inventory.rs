//! Project-wide storage of run/debug configurations and recipes.

use std::{
    borrow::Cow,
    path::PathBuf,
    sync::Arc,
};

use collections::HashMap;
use gpui::{App, AppContext, Entity};
use run_and_debug::configuration::{Recipe, Recipes, RunnableConfiguration, ConfigurationTemplates};
use settings::{InvalidSettingsError, parse_json_with_comments};
use util::rel_path::RelPath;
use worktree::WorktreeId;

/// Inventory of run/debug configurations and recipes for the project
pub struct RunAndDebugInventory {
    /// Configurations from settings files
    configurations: InventoryFor<RunnableConfiguration>,
    /// Recipes from settings files
    recipes: InventoryFor<Recipe>,
}

impl std::fmt::Debug for RunAndDebugInventory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunAndDebugInventory")
            .field("configurations", &self.configurations)
            .field("recipes", &self.recipes)
            .finish()
    }
}

#[derive(Debug)]
struct InventoryFor<T> {
    global: HashMap<PathBuf, Vec<T>>,
    worktree: HashMap<WorktreeId, HashMap<Arc<RelPath>, Vec<T>>>,
}

impl<T> Default for InventoryFor<T> {
    fn default() -> Self {
        Self {
            global: HashMap::default(),
            worktree: HashMap::default(),
        }
    }
}

impl<T: Clone> InventoryFor<T> {
    fn worktree_items(
        &self,
        worktree: WorktreeId,
    ) -> impl '_ + Iterator<Item = (RunAndDebugSourceKind, T)> {
        self.worktree
            .get(&worktree)
            .into_iter()
            .flatten()
            .flat_map(|(directory, items)| {
                items.iter().map(move |item| (directory, item))
            })
            .map(move |(directory, item)| {
                (
                    RunAndDebugSourceKind::Worktree {
                        id: worktree,
                        directory_in_worktree: directory.clone(),
                        id_base: Cow::Owned(format!(
                            "local worktree from directory {directory:?}"
                        )),
                    },
                    item.clone(),
                )
            })
    }

    fn global_items(&self) -> impl '_ + Iterator<Item = (RunAndDebugSourceKind, T)> {
        self.global.iter().flat_map(|(file_path, items)| {
            items.iter().map(|item| {
                (
                    RunAndDebugSourceKind::AbsPath {
                        id_base: Cow::Borrowed("global"),
                        abs_path: file_path.clone(),
                    },
                    item.clone(),
                )
            })
        })
    }

    fn update(
        &mut self,
        location: SettingsLocation,
        items: Vec<T>,
    ) {
        match location {
            SettingsLocation::Global(path) => {
                if items.is_empty() {
                    self.global.remove(&path);
                } else {
                    self.global.insert(path, items);
                }
            }
            SettingsLocation::Worktree {
                worktree_id,
                directory_in_worktree,
            } => {
                let worktree_items = self.worktree.entry(worktree_id).or_default();
                if items.is_empty() {
                    worktree_items.remove(&directory_in_worktree);
                    if worktree_items.is_empty() {
                        self.worktree.remove(&worktree_id);
                    }
                } else {
                    worktree_items.insert(directory_in_worktree, items);
                }
            }
        }
    }
}

/// Kind of source for configurations/recipes
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RunAndDebugSourceKind {
    /// User-created one-off configurations
    UserInput,
    /// Items from .zed/configurations.json or .zed/recipes.json
    Worktree {
        id: WorktreeId,
        directory_in_worktree: Arc<RelPath>,
        id_base: Cow<'static, str>,
    },
    /// Global items from ~/.config/zed/configurations.json or ~/.config/zed/recipes.json
    AbsPath {
        id_base: Cow<'static, str>,
        abs_path: PathBuf,
    },
}

/// Location of settings file
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsLocation {
    Global(PathBuf),
    Worktree {
        worktree_id: WorktreeId,
        directory_in_worktree: Arc<RelPath>,
    },
}

impl RunAndDebugInventory {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| {
            let mut inventory = Self {
                configurations: InventoryFor::default(),
                recipes: InventoryFor::default(),
            };
            
            inventory
        })
    }

    /// List all available configurations
    pub fn list_configurations(
        &self,
        worktree: Option<WorktreeId>,
    ) -> Vec<(RunAndDebugSourceKind, RunnableConfiguration)> {
        let mut configurations = Vec::new();

        // Add configurations from worktree
        if let Some(worktree_id) = worktree {
            configurations.extend(self.configurations.worktree_items(worktree_id));
        }

        // Add global configurations
        configurations.extend(self.configurations.global_items());

        configurations
    }

    /// List all available recipes
    pub fn list_recipes(
        &self,
        worktree: Option<WorktreeId>,
    ) -> Vec<(RunAndDebugSourceKind, Recipe)> {
        let mut recipes = Vec::new();

        // Add recipes from worktree (they have priority)
        if let Some(worktree_id) = worktree {
            recipes.extend(self.recipes.worktree_items(worktree_id));
        }

        // Add global recipes
        recipes.extend(self.recipes.global_items());

        recipes
    }

    /// Get a recipe by name, searching worktree-specific recipes first, then global
    pub fn get_recipe(&self, name: &str, worktree: Option<WorktreeId>) -> Option<Recipe> {
        // First check worktree-specific recipes if we have a worktree
        if let Some(worktree_id) = worktree {
            for (_, recipe) in self.recipes.worktree_items(worktree_id) {
                if recipe.name == name {
                    return Some(recipe);
                }
            }
        }
        
        // Then check global recipes
        for (_, recipe) in self.recipes.global_items() {
            if recipe.name == name {
                return Some(recipe);
            }
        }
        
        None
    }

    /// Update configurations from a settings file
    pub fn update_configurations(
        &mut self,
        location: SettingsLocation,
        configurations: Vec<RunnableConfiguration>,
    ) {
        self.configurations.update(location, configurations);
    }

    /// Update recipes from a settings file
    pub fn update_recipes(
        &mut self,
        location: SettingsLocation,
        recipes: Vec<Recipe>,
    ) {
        self.recipes.update(location, recipes);
    }

    // /// Clear configurations and recipes for a specific worktree
    // pub fn clear_worktree(&mut self, worktree_id: WorktreeId) {
    //     self.configurations.worktree.remove(&worktree_id);
    //     self.recipes.worktree.remove(&worktree_id);
    //     self.last_scheduled_configurations.retain(|(source, _)| {
    //         !matches!(source, RunAndDebugSourceKind::Worktree { id, .. } if id == &worktree_id)
    //     });
    // }
    //
    // /// Delete a previously used configuration from history
    // pub fn delete_previously_used(&mut self, id: &run_and_debug::configuration::ConfigurationId) {
    //     self.last_scheduled_configurations.retain(|(_, config)| &config.id != id);
    // }
}

/// Parse configurations from JSON
pub fn parse_configuration_file(
    content: String,
) -> Result<Vec<RunnableConfiguration>, InvalidSettingsError> {
    let json_value = parse_json_with_comments::<serde_json::Value>(&content)
        .map_err(|err| InvalidSettingsError::InvalidConfigurationFile(err.to_string()))?;
    let configurations: ConfigurationTemplates = serde_json::from_value(json_value)
        .map_err(|err| InvalidSettingsError::InvalidConfigurationFile(err.to_string()))?;
    Ok(configurations.0)
}

/// Parse recipes from JSON
pub fn parse_recipe_file(content: String) -> Result<Vec<Recipe>, InvalidSettingsError> {
    let json_str = content.trim();
    if json_str.is_empty() {
        return Ok(Vec::new());
    }
    
    parse_json_with_comments::<Recipes>(json_str)
        .map(|recipes| recipes.0)
        .map_err(|err| InvalidSettingsError::InvalidRecipeFile(format!("Failed to parse recipes: {}", err)))
}
