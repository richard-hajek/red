use collections::HashMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use crate::recipe::{Recipe, Recipes};

/// A configuration that specifies how to execute a program using a recipe
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunnableConfiguration {
    /// Human-readable name of the configuration
    pub label: String,
    
    /// Recipe name to use (e.g., "python", "cargo", "node")
    /// The recipe defines the available execution modes
    pub recipe: String,
    
    /// Variables to pass to the recipe template
    /// These are substituted using $VAR syntax in recipe commands
    #[serde(default)]
    pub variables: HashMap<String, String>,
    
    /// Working directory for execution (optional)
    /// Supports $VAR substitution
    #[serde(default)]
    pub cwd: Option<String>,
    
    /// Environment variables to set
    /// Values support $VAR substitution
    #[serde(default)]
    pub env: HashMap<String, String>,
    
    /// Tags for categorizing configurations (optional)
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A collection of configurations
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigurationTemplates(pub Vec<RunnableConfiguration>);