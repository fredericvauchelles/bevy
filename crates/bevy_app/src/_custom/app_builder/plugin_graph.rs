//! [`PluginGraph`] is a graph of plugin dependencies, required to build an appropriate order of
//! plugin execution

use alloc::boxed::Box;
use bevy_app::*;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use petgraph::prelude::NodeIndex;
use thiserror::Error;
#[cfg(feature = "trace")]
use tracing::*;

/// Error when getting a plugin in the [`PluginGraph`]
#[derive(Debug, Error)]
pub enum GetPluginError {
    /// The plugin with provided id is not of the expected type
    #[error("The plugin type with id {0} is not of the expected type {1}")]
    InvalidType(PluginId, &'static str),
    /// The plugin with given id was not added
    #[error("The plugin with id {0} was not added")]
    PluginNotAdded(PluginId),
}

#[derive(Default)]
pub struct PluginGraph {
    plugins: HashMap<NodeIndex, Box<dyn Plugin>>,
    id2node: HashMap<PluginId, NodeIndex>,
    graph: petgraph::Graph<PluginId, ()>,
}

struct PluginGraphPluginGroup;
impl PluginGroup for PluginGraphPluginGroup {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
    }
}

impl PluginGraph {
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        let plugins = plugins.into_boxed_vec();
        for plugin in plugins {
            let id = plugin.id();
            if self.id2node.contains_key(&id) {
                continue;
            }

            let node = self.graph.add_node(id.clone());
            self.plugins.insert(node, plugin);
            self.id2node.insert(id, node);
        }
        self
    }

    pub fn upset_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        let plugins = plugins.into_boxed_vec();
        for plugin in plugins {
            let id = plugin.id();
            if let Some(node) = self.id2node.get(&id) {
                *self.plugins.get_mut(node).unwrap() = plugin;
            } else {
                let node = self.graph.add_node(id.clone());
                self.plugins.insert(node, plugin);
                self.id2node.insert(id, node);
            }
        }
        self
    }

    pub fn contains_plugin_id(&self, id: &PluginId) -> bool {
        self.id2node.contains_key(id)
    }

    pub fn contains_plugin<P: Plugin>(&self) -> bool {
        self.contains_plugin_id(&PluginId::of::<P>())
    }

    pub fn get_plugin<P: Plugin>(&self, id: &PluginId) -> Result<&P, GetPluginError> {
        let Some(node) = self.id2node.get(id) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = self.plugins.get(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = plugin.downcast_ref::<P>() else {
            return Err(GetPluginError::InvalidType(
                id.clone(),
                core::any::type_name::<P>(),
            ));
        };
        Ok(plugin)
    }

    pub fn get_plugin_mut<P: Plugin>(&mut self, id: &PluginId) -> Result<&mut P, GetPluginError> {
        let Some(node) = self.id2node.get(id) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = self.plugins.get_mut(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = plugin.downcast_mut::<P>() else {
            return Err(GetPluginError::InvalidType(
                id.clone(),
                core::any::type_name::<P>(),
            ));
        };
        Ok(plugin)
    }

    pub fn try_into_plugin_group_builder(self) -> Result<PluginGroupBuilder, BevyError> {
        self.try_into()
    }

    pub fn iter_plugins(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins.values().map(core::ops::Deref::deref)
    }

    fn add_plugin_edges(mut self) -> Result<PluginGraph, PluginGraphBuildError> {
        // add dependencies (edges)
        for plugin in self.plugins.values() {
            let plugin_id = plugin.id();
            let this_plugin = *self.id2node.get(&plugin_id).unwrap();
            for from in &*plugin.build_after() {
                let (from, is_required) = match from {
                    PluginDependency::Required(id) => (id, true),
                    PluginDependency::Optional(id) => (id, false),
                };
                if let Some(from) = self.id2node.get(from) {
                    self.graph.add_edge(*from, this_plugin, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(from.clone()))?
                } else {
                    #[cfg(feature = "trace")]
                    warn!(
                        "Optional dependency not found: {plugin_id} should build after {from} (not found)"
                    );
                }
            }
            for to in &*plugin.build_before() {
                let (to, is_required) = match to {
                    PluginDependency::Required(id) => (id, true),
                    PluginDependency::Optional(id) => (id, false),
                };
                if let Some(to) = self.id2node.get(to) {
                    self.graph.add_edge(this_plugin, *to, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(to.clone()))?
                } else {
                    #[cfg(feature = "trace")]
                    warn!(
                        "Optional dependency not found: {plugin_id} should build before {to} (not found)"
                    );
                }
            }
        }

        Ok(self)
    }

    fn try_into_sorted_plugins(self) -> Result<bevy_platform::prelude::Vec<Box<dyn Plugin>>, PluginGraphBuildError> {
        let mut value = self.add_plugin_edges()?;

        let sorted = petgraph::algo::toposort(&value.graph, None).map_err(|err| {
            PluginGraphBuildError::CircularDependency(
                value.plugins.get(&err.node_id()).unwrap().id(),
            )
        })?
            .into_iter()
            .map(|n| value.plugins.remove(&n).unwrap());

        Ok(sorted.collect())
    }
}

#[derive(Debug, Error)]
pub enum PluginGraphBuildError {
    #[error("Missing required plugin: '{0}'")]
    MissingRequiredPlugin(PluginId),
    #[error("Circular dependency detected with plugin: '{0}'")]
    CircularDependency(PluginId),
}

impl TryFrom<PluginGraph> for PluginGroupBuilder {
    type Error = BevyError;

    fn try_from(value: PluginGraph) -> Result<Self, Self::Error> {
        let sorted = value.try_into_sorted_plugins()?;

        #[cfg(feature = "trace")]
        {
            use bevy_platform::prelude::*;
            trace!("Sorted plugins: {}", sorted.iter().map(|n| n.id().to_string()).collect::<Vec<_>>().join(", "));
        }
        let mut result = PluginGraphPluginGroup.build();
        result.extend(sorted);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::Cow;
    use bevy_platform::prelude::*;

    struct PluginA;
    impl Plugin for PluginA {
        fn build(&self, _: &mut App) {}
        fn build_before(&'_ self) -> Cow<'_, [PluginDependency]> {
            plugin_deps!(PluginB).into()
        }
    }
    struct PluginB;
    impl Plugin for PluginB {
        fn build(&self, _: &mut App) {}
    }
    struct PluginC;
    impl Plugin for PluginC {
        fn build(&self, _: &mut App) {}
        fn build_after(&'_ self) -> Cow<'_, [PluginDependency]> {
            plugin_deps!(PluginB).into()
        }
    }

    #[test]
    fn build_order() {
        fn test<M>(plugins: impl Plugins<M>) {
            let mut plugin_graph = PluginGraph::default();
            plugin_graph.add_plugins(plugins);
            let plugins = plugin_graph.try_into_sorted_plugins().unwrap();
            let plugin_ids = plugins.iter().map(|p| p.id()).collect::<Vec<_>>();

            assert_eq!(vec![PluginId::of::<PluginA>(), PluginId::of::<PluginB>(), PluginId::of::<PluginC>()], plugin_ids);
        }

        test((PluginA, PluginB, PluginC));
        test((PluginC, PluginA, PluginB));
        test((PluginB, PluginC, PluginA));
        test((PluginA, PluginC, PluginB));
        test((PluginB, PluginA, PluginC));
        test((PluginC, PluginB, PluginA));
    }
}