//! App builder is used to build an app with the appropriate plugin order

use crate::*;
use alloc::vec::Vec;
use bevy_ecs::error::*;
use bevy_ecs::prelude::BevyError;
use core::borrow::Borrow;
use plugin_graph::PluginGraph;

mod plugin_graph;

/// Prelude for [`AppBuilder`]
pub mod prelude {
    pub use super::AppBuilder;
}

pub use plugin_graph::GetPluginError;

/// App builder is used to build an app with the appropriate plugin order
pub struct AppBuilder {
    plugin_graph: PluginGraph,
}
impl AppBuilder {
    /// Add plugins to the graph
    ///
    /// If a plugin with the same [`PluginId`] is already added, then it will be overwritten.
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        self.plugin_graph.add_plugins(plugins);
        self
    }

    /// Add a build fn as a plugin with provided id.
    pub fn add_build_with_id<F: 'static + Sync + Send + Fn(&mut App)>(
        &mut self,
        build: F,
        before: impl Into<Vec<PluginDependency>>,
        after: impl Into<Vec<PluginDependency>>,
        id: PluginId,
    ) -> &mut Self {
        struct FnPlugin<F>(F, Vec<PluginDependency>, Vec<PluginDependency>, PluginId);
        impl<F: 'static + Sync + Send + Fn(&mut App)> Plugin for FnPlugin<F> {
            fn build(&self, app: &mut App) {
                self.0(app)
            }
            fn build_after(&self) -> alloc::borrow::Cow<'_, [PluginDependency]> {
                (&*self.2).into()
            }
            fn build_before(&self) -> alloc::borrow::Cow<'_, [PluginDependency]> {
                (&*self.1).into()
            }
            fn id(&self) -> PluginId {
                self.3.clone()
            }
        }
        self.add_plugins(FnPlugin(build, before.into(), after.into(), id.clone()))
    }

    /// Add a build fn as a plugin with a random id.
    pub fn add_build<F: 'static + Sync + Send + Fn(&mut App)>(
        &mut self,
        build: F,
        before: impl Into<Vec<PluginDependency>>,
        after: impl Into<Vec<PluginDependency>>,
    ) -> PluginId {
        let id = PluginId::random();
        self.add_build_with_id(build, before.into(), after.into(), id.clone());
        id
    }

    /// Update or insert plugins
    pub fn upset_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        self.plugin_graph.upset_plugins(plugins);
        self
    }

    /// get a plugin
    pub fn get_plugin<P: Plugin>(&self, id: impl Borrow<PluginId>) -> Result<&P, GetPluginError> {
        self.plugin_graph.get_plugin::<P>(id.borrow())
    }

    /// get a plugin
    pub fn get_plugin_mut<P: Plugin>(
        &mut self,
        id: impl Borrow<PluginId>,
    ) -> Result<&mut P, GetPluginError> {
        self.plugin_graph.get_plugin_mut::<P>(id.borrow())
    }

    /// Checks if a plugin is already added
    pub fn contains_plugin_id(&self, id: &PluginId) -> bool {
        self.plugin_graph.contains_plugin_id(id)
    }

    /// Checks if a plugin is already added
    pub fn contains_plugin<P: Plugin>(&self) -> bool {
        self.plugin_graph.contains_plugin::<P>()
    }

    /// Returns an empty [`AppBuilder`]
    pub fn empty() -> Self {
        let value = Self {
            plugin_graph: PluginGraph::default(),
        };
        value
    }

    /// Sets the error handler of the app
    pub fn set_error_handler(&mut self, handler: fn(BevyError, ErrorContext)) -> &mut Self {
        self.add_build(
            move |app| {
                app.set_error_handler(handler);
            },
            [],
            [],
        );
        self
    }

    /// Build the app and run it
    pub fn run(self) -> Result<AppExit, BevyError> {
        self.run_with(|mut app| app.run())
    }

    /// Build the app and then execute the runner function
    pub fn run_with(mut self, runner: impl FnOnce(App) -> AppExit) -> Result<AppExit, BevyError> {
        let pre_builds = self
            .plugin_graph
            .iter_plugins()
            .flat_map(|p| p.pre_build().into_iter())
            .collect::<Vec<_>>();

        for pre_build in pre_builds {
            pre_build(&mut self);
        }

        let mut app = App::new();
        let plugin_group = self.plugin_graph.try_into_plugin_group_builder()?;

        plugin_group.finish(&mut app);
        app.finish();
        app.cleanup();

        Ok(runner(app))
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        let mut result = Self::empty();
        result.set_error_handler(error);
        result
    }
}
