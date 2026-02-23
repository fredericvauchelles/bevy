//! App builder is used to build an app with the appropriate plugin order

use crate::*;
pub use bevy_app::app_builder::plugin_graph::UpdatePluginDependency;
use bevy_ecs::error::*;
use bevy_ecs::prelude::BevyError;
use bevy_platform::prelude::*;
use core::borrow::Borrow;
use plugin_graph::PluginGraph;
use std::ops::Deref;

mod plugin_graph;

/// Prelude for [`AppBuilder`]
pub mod prelude {
    pub use super::{AppBuilder, UpdatePluginDependency};
    pub use crate::_custom::app_builder::plugin_graph::PluginDependencyPatch;
}

pub use plugin_graph::GetPluginError;
pub use plugin_graph::PluginDependencyPatch;

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

    /// Add plugins to the graph as a single node with id `alias`
    ///
    /// If a plugin with the same [`PluginId`] is already added, then it will be overwritten.
    ///
    /// Plugins will be built in provided order
    ///
    /// You can use it for existing plugin group that do not use the dependency system
    /// but are in a manually defined order
    pub fn add_plugins_as_alias<M>(&mut self, alias: impl Into<PluginId>, plugins: impl Plugins<M>) -> &mut Self {
        self.plugin_graph.add_plugins_as_alias(alias.into(), plugins);
        self
    }

    /// Add a build fn as a plugin with provided id.
    pub fn add_build_with_id<F: 'static + Sync + Send + Fn(&mut App)>(
        &mut self,
        build: F,
        before: impl Into<Vec<PluginDependency>>,
        after: impl Into<Vec<PluginDependency>>,
        id: impl Into<PluginId>,
    ) -> &mut Self {
        struct FnPlugin<F>(F, Vec<PluginDependency>, Vec<PluginDependency>, PluginId);
        impl<F: 'static + Sync + Send + Fn(&mut App)> Plugin for FnPlugin<F> {
            fn build(&self, app: &mut App) {
                self.0(app)
            }
            fn id(&self) -> PluginId {
                self.3.clone()
            }
            fn build_after(&self) -> alloc::borrow::Cow<'_, [PluginDependency]> {
                (&*self.2).into()
            }
            fn build_before(&self) -> alloc::borrow::Cow<'_, [PluginDependency]> {
                (&*self.1).into()
            }
        }
        self.add_plugins(FnPlugin(build, before.into(), after.into(), id.into()))
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

    /// Patch plugins dependencies
    ///
    /// This is useful to patch dependencies of a third party plugin so it matches
    /// the project's plugin dependencies
    pub fn patch_plugin_dependencies(&mut self, patch: &PluginDependencyPatch) -> &mut Self {
        self.plugin_graph.patch_plugin_dependencies(patch);
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
        self.pre_build_recursive();

        let mut app = App::new();
        let plugin_group = self.plugin_graph.try_into_plugin_group_builder()?;

        plugin_group.finish(&mut app);
        app.finish();
        app.cleanup();

        Ok(runner(app))
    }

    fn pre_build_recursive(&mut self) -> bevy_platform::collections::HashSet<PluginId> {
        let mut pre_built_plugins = bevy_platform::collections::HashSet::new();
        loop {
            let pre_builds = self
                .plugin_graph
                .iter_plugins()
                .flat_map(|p| p.plugins())
                .filter(|p| !pre_built_plugins.contains(&p.id()))
                .flat_map(|p| p.deref().pre_build().into_iter().map(|pre_build| (p.id(), pre_build)))
                .collect::<Vec<_>>();

            if pre_builds.is_empty() {
                break;
            }

            for (id, pre_build) in pre_builds {
                pre_build(self);
                log::trace!("Pre built plugin: {id}");
                pre_built_plugins.insert(id);
            }
        }
        pre_built_plugins
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        let mut result = Self::empty();
        result.set_error_handler(error);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_platform::collections::*;

    struct PluginA;
    impl Plugin for PluginA {
        fn build(&self, _: &mut App) {}
        fn pre_build(&self) -> Option<Box<dyn FnOnce(&mut AppBuilder)>> {
            Some(Box::new(|app| { app.add_plugins(PluginB); }))
        }
    }
    struct PluginB;
    impl Plugin for PluginB {
        fn build(&self, _: &mut App) {}
        fn pre_build(&self) -> Option<Box<dyn FnOnce(&mut AppBuilder)>> {
            Some(Box::new(|app| { app.add_plugins(PluginC); }))
        }
    }
    struct PluginC;
    impl Plugin for PluginC {
        fn build(&self, _: &mut App) {}
    }

    #[test]
    fn pre_build_recursive() {
        let mut app = AppBuilder::default();
        app.add_plugins(PluginA);

        let pre_built = app.pre_build_recursive();
        let plugin_ids = app.plugin_graph.iter_plugins().flat_map(|p| p.plugins()).map(|entry| entry.deref().id()).collect::<HashSet<_>>();

        assert_eq!(pre_built, vec![PluginId::of::<PluginA>(), PluginId::of::<PluginB>()].into_iter().collect());
        assert!(plugin_ids.is_superset(&vec![PluginId::of::<PluginA>(), PluginId::of::<PluginB>(), PluginId::of::<PluginC>()].into_iter().collect()));
    }
}