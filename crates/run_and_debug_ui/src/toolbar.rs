use crate::{execution::{prepare_execution, PreparedExecution}, set_selected_configuration, SelectedConfiguration};
use gpui::{
    Context, EventEmitter, IntoElement, ParentElement, Render, Styled, Subscription, WeakEntity,
    Window,
};
use project::RunAndDebugSourceKind;
use run_and_debug::configuration::RunnableConfiguration;
use task;
use ui::{prelude::*, ButtonStyle, ContextMenu, DropdownMenu, IconButton, IconName, Tooltip};
use workspace::{ToolbarItemEvent, ToolbarItemLocation, ToolbarItemView, Workspace};

pub struct RunAndDebugToolbar {
    workspace: WeakEntity<Workspace>,
    selected_configuration: Option<(RunAndDebugSourceKind, RunnableConfiguration)>,
    available_configurations: Vec<(RunAndDebugSourceKind, RunnableConfiguration)>,
    _subscriptions: Vec<Subscription>,
}

impl RunAndDebugToolbar {
    pub fn new(workspace: WeakEntity<Workspace>, cx: &mut Context<Self>) -> Self {
        let toolbar = Self {
            workspace: workspace.clone(),
            selected_configuration: None,
            available_configurations: Vec::new(),
            _subscriptions: Vec::new(),
        };
        
        let weak_self = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            cx.update(|cx| {
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, |this, cx| {
                        this.setup_subscriptions(cx);
                        this.load_configurations(cx);
                    });
                }
            }).ok();
        }).detach();
        
        toolbar
    }
    
    fn setup_subscriptions(&mut self, cx: &mut Context<Self>) {
        self._subscriptions.push(cx.observe_global::<SelectedConfiguration>(
            |toolbar, cx| {
                let selected = cx.global::<SelectedConfiguration>().selection.clone();
                if let Some((source, template)) = selected {
                    log::info!("Toolbar: Configuration changed to '{}'", template.label);
                    toolbar.selected_configuration = Some((source, (*template).clone()));
                    cx.notify();
                }
            },
        ));
        
        if let Some(workspace_entity) = self.workspace.upgrade() {
            let project = workspace_entity.read(cx).project().clone();
            let config_store_opt = project.read(cx).configuration_store().cloned();
            let inventory_opt = config_store_opt.as_ref()
                .and_then(|store| store.read(cx).run_and_debug_inventory().cloned());
            
            self._subscriptions.push(cx.observe(&project, |toolbar, _project, cx| {
                toolbar.load_configurations(cx);
            }));
            
            if let Some(config_store) = config_store_opt {
                self._subscriptions.push(cx.observe(&config_store, |toolbar, _store, cx| {
                    toolbar.load_configurations(cx);
                }));
            }
            
            if let Some(inventory) = inventory_opt {
                self._subscriptions.push(cx.observe(&inventory, |toolbar, _inventory, cx| {
                    toolbar.load_configurations(cx);
                }));
            }
        }
    }

    fn load_configurations(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.upgrade() {
            let project = workspace.read(cx).project().clone();
            if let Some(config_store) = project.read(cx).configuration_store() {
                if let Some(inventory) = config_store.read(cx).run_and_debug_inventory() {
                    let mut all_configurations = Vec::new();
                    let worktrees: Vec<_> = project.read(cx).worktrees(cx).collect();
                    
                    if worktrees.is_empty() {
                        all_configurations = inventory.read(cx).list_configurations(None);
                    } else {
                        use std::collections::HashSet;
                        let mut seen = HashSet::new();
                        
                        for worktree in worktrees {
                            let worktree_id = worktree.read(cx).id();
                            let configs = inventory.read(cx).list_configurations(Some(worktree_id));
                            for config in configs {
                                let key = config.1.label.to_string();
                                if seen.insert(key) {
                                    all_configurations.push(config);
                                }
                            }
                        }
                    }
                    
                    self.available_configurations = all_configurations;
                    
                    if self.selected_configuration.is_none() && !self.available_configurations.is_empty() {
                        self.selected_configuration = Some(self.available_configurations[0].clone());
                    }
                    
                    cx.notify();
                }
            }
        }
    }

    /// Execute a specific mode of the selected configuration
    fn execute_mode(&mut self, mode_name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_source, config)) = &self.selected_configuration else {
            return;
        };

        let Some(workspace) = self.workspace.upgrade() else {
            log::error!("Workspace not available");
            return;
        };
        
        let config = config.clone();
        let mode_name = mode_name.to_string();
        
        workspace.update(cx, |workspace, cx| {
            let project = workspace.project().clone();
            
            let recipe_name = &config.recipe;
            log::info!("Using recipe: {}", recipe_name);
            
            let Some(config_store) = project.read(cx).configuration_store() else {
                log::error!("Configuration store not available");
                return;
            };
            
            let worktree_id = project.read(cx).worktrees(cx).next()
                .map(|wt| wt.read(cx).id());
            
            let Some(recipe) = config_store.read(cx).get_recipe(recipe_name, worktree_id, cx) else {
                log::error!("Recipe '{}' not found", recipe_name);
                return;
            };

            let task_store = project.read(cx).task_store().clone();
            let active_buffer = workspace.active_item(cx)
                .and_then(|item| item.act_as::<language::Buffer>(cx));
            
            let location = active_buffer.as_ref().and_then(|buffer| {
                buffer.read(cx).file().map(|_file| {
                    let anchor = buffer.read(cx).anchor_before(0);
                    language::Location {
                        buffer: buffer.clone(),
                        range: anchor..anchor,
                    }
                })
            });

            let recipe = recipe.clone();
            
            // Spawn async task to get context and execute
            cx.spawn_in(window, async move |workspace, cx| {
                let task_context = if let Some(location) = location {
                    log::info!("Getting task context for location...");
                    let context_task = task_store.update(cx, |store, cx| {
                        store.task_context_for_location(
                            Default::default(),
                            location,
                            cx
                        )
                    }).ok();
                    
                    if let Some(task) = context_task {
                        log::info!("Awaiting task context...");
                        let result = task.await;
                        log::info!("Got task context: {:?}", result.is_some());
                        result
                    } else {
                        log::warn!("Failed to create context task");
                        None
                    }
                } else {
                    log::warn!("No location available, using empty task context");
                    None
                };
                
                let mut task_context = task_context.unwrap_or_else(|| {
                    log::warn!("Using default/empty TaskContext");
                    Default::default()
                });
                
                // Inject configuration variables into task context
                log::info!("Injecting {} configuration variables into task context", config.variables.len());
                for (var_name, var_value) in &config.variables {
                    log::info!("  Injecting variable: {} = {}", var_name, var_value);
                    task_context.task_variables.insert(
                        task::VariableName::Custom(std::borrow::Cow::Owned(var_name.clone())),
                        var_value.clone(),
                    );
                }
                
                // Prepare execution
                let prepared = match prepare_execution(&mode_name, &recipe, &task_context) {
                    Ok(p) => p,
                    Err(e) => {
                        log::error!("Failed to prepare execution for mode '{}': {}", mode_name, e);
                        return;
                    }
                };
                
                // Execute based on prepared type
                match prepared {
                    PreparedExecution::Task(resolved_task) => {
                        log::info!("Spawning task: label='{}'", resolved_task.display_label());
                        workspace.update_in(cx, |workspace, window, cx| {
                            // Use Zed's built-in task scheduling!
                            log::info!("Task to run: {:?}", resolved_task);
                            workspace.schedule_resolved_task(
                                project::TaskSourceKind::UserInput,
                                resolved_task,
                                false, // allow_concurrent_runs - use task template's setting
                                window,
                                cx,
                            );
                        }).ok();
                    }
                    PreparedExecution::Debug(scenario) => {
                        log::info!("Debug scenario to run: {:?}", scenario);
                        workspace.update_in(cx, |workspace, window, cx| {
                            workspace.start_debug_session(
                                scenario,
                                task_context,
                                active_buffer,
                                worktree_id,
                                window,
                                cx,
                            );
                        }).ok();
                    }
                }
            }).detach();
        });
    }

    /// Get available modes for the selected configuration's recipe
    fn get_available_modes(&self, cx: &Context<Self>) -> Vec<String> {
        let Some((_source, config)) = &self.selected_configuration else {
            log::info!("get_available_modes: No configuration selected");
            return vec![];
        };
        
        let Some(workspace) = self.workspace.upgrade() else {
            log::info!("get_available_modes: Workspace not available");
            return vec![];
        };
        
        let project = workspace.read(cx).project().clone();
        let Some(config_store) = project.read(cx).configuration_store() else {
            log::info!("get_available_modes: Config store not available");
            return vec![];
        };
        
        let worktree_id = project.read(cx).worktrees(cx).next()
            .map(|wt| wt.read(cx).id());
        
        let Some(recipe) = config_store.read(cx).get_recipe(&config.recipe, worktree_id, cx) else {
            log::info!("get_available_modes: Recipe '{}' not found", config.recipe);
            return vec![];
        };
        
        let modes = recipe.available_modes().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        modes
    }
    
    /// Render a mode button
    fn render_mode_button(
        &self,
        mode_name: &str,
        icon: IconName,
        color: Color,
        tooltip_text: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mode_name = mode_name.to_string();
        let tooltip_text = tooltip_text.to_string();
        IconButton::new(format!("mode-{}", mode_name), icon)
            .icon_color(color)
            .style(ButtonStyle::Filled)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.execute_mode(&mode_name, window, cx);
            }))
            .tooltip(move |window, cx| Tooltip::text(tooltip_text.clone())(window, cx))
    }
}

impl EventEmitter<ToolbarItemEvent> for RunAndDebugToolbar {}

impl ToolbarItemView for RunAndDebugToolbar {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn workspace::ItemHandle>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ToolbarItemLocation {
        if self.available_configurations.is_empty() {
            self.load_configurations(cx);
        }
        ToolbarItemLocation::PrimaryRight
    }
}

impl Render for RunAndDebugToolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_label = self
            .selected_configuration
            .as_ref()
            .map(|(_, config)| config.label.to_string())
            .unwrap_or_else(|| "No configuration".to_string());

        let has_selection = self.selected_configuration.is_some();
        let available_modes = self.get_available_modes(cx);
        
        // Build the dropdown menu
        let weak = cx.weak_entity();
        let configurations = self.available_configurations.clone();
        
        let dropdown_menu = ContextMenu::build(window, cx, move |mut menu, _, _cx| {
            if configurations.is_empty() {
                menu = menu.entry("No configurations available", None, |_, _| {});
            } else {
                for (source, config) in configurations {
                    let weak = weak.clone();
                    let source = source.clone();
                    let config = config.clone();
                    menu = menu.entry(config.label.clone(), None, move |_, cx| {
                        if let Some(this) = weak.upgrade() {
                            this.update(cx, |this, cx| {
                                this.selected_configuration = Some((source.clone(), config.clone()));
                                set_selected_configuration(source.clone(), config.clone(), cx);
                                cx.notify();
                            });
                        }
                    });
                }
            }
            menu
        });

        h_flex()
            .gap_2()
            .child(
                DropdownMenu::new("select-configuration", selected_label, dropdown_menu)
                    .style(ui::DropdownStyle::Subtle)
            )
            .child(div().w_1())
            // Run button (always show if config selected)
            .when_else(has_selection && available_modes.contains(&"run".to_string()), |this| {
                this.child(self.render_mode_button("run", IconName::PlayOutlined, Color::Success, "Run", cx))
                }, |this| {
                this.child(self.render_mode_button("run", IconName::PlayOutlined, Color::Disabled, "Run", cx))
                }
            )
            .when_else(has_selection && available_modes.contains(&"debug".to_string()), |this| {
                this.child(self.render_mode_button("debug", IconName::Debug, Color::Warning, "Debug", cx))
                }, |this| {
                this.child(self.render_mode_button("debug", IconName::Debug, Color::Disabled, "Debug", cx))
                }
            )
            .child(div().w_4())
    }
}
