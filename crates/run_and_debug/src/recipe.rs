use collections::HashMap;
use serde::{Deserialize, Serialize};
use task::{DebugScenario, TaskTemplate};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExecutionMode {
    Task(TaskTemplate),
    Debug(DebugScenario),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub modes: HashMap<String, ExecutionMode>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Recipe {
    pub fn get_mode(&self, mode_name: &str) -> Option<&ExecutionMode> {
        self.modes.get(mode_name)
    }
    
    /// Get all available mode names
    pub fn available_modes(&self) -> Vec<&str> {
        self.modes.keys().map(|s| s.as_str()).collect()
    }
    
    /// Check if a specific mode is available
    pub fn has_mode(&self, mode_name: &str) -> bool {
        self.modes.contains_key(mode_name)
    }
}

/// A collection of recipes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Recipes(pub Vec<Recipe>);
