//! [`PluginGraph`] is a graph of plugin dependencies, required to build an appropriate order of
//! plugin execution

use bevy_app::*;
use bevy_ecs::prelude::*;
use bevy_platform::collections::*;
use bevy_platform::prelude::*;
use core::any::TypeId;
use core::ops::Deref;
pub use entry::GetPluginError;
use entry::PluginEntry;
use log::error;
pub use patch::PluginDependencyPatch;
use petgraph::prelude::*;
use thiserror::Error;

mod entry;
mod patch;

#[derive(Default)]
pub struct PluginGraph {
    node2entry: HashMap<NodeIndex, PluginEntry>,
    id2node: HashMap<PluginId, NodeIndex>,
    node2id: HashMap<NodeIndex, PluginId>,
    /// aliases redirect the key [`PluginId`] to the value [`PluginId`]
    aliases: HashMap<PluginId, PluginId>,
    graph: Graph<PluginId, ()>,
}

/// Describe how to update a plugin dependency
#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum UpdatePluginDependency {
    /// Add dependency in `build_before`
    AddBefore(PluginDependency),
    /// Add dependency in `build_after`
    AddAfter(PluginDependency),
    /// Remove dependency in `build_before`
    RemoveBefore(PluginDependency),
    /// Remove dependency in `build_after`
    RemoveAfter(PluginDependency),
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
            self.node2entry.insert(node, PluginEntry::new(vec![plugin]));
            self.id2node.insert(id.clone(), node);
            self.node2id.insert(node, id);
        }
        self
    }

    /// Insert plugins in a single node with provided `alias`
    ///
    /// Plugins will be built in provided order
    ///
    /// You can use it for existing plugin group that do not use the dependency system
    /// but are in a manually defined order
    pub fn add_plugins_as_alias<M>(
        &mut self,
        alias: PluginId,
        plugins: impl Plugins<M>,
    ) -> &mut Self {
        let plugins = plugins.into_boxed_vec();
        if self.id2node.contains_key(&alias) {
            error!(
                "Tried to overwrite an existing plugin: {alias}.\
             Skipping adding plugins ({}) as alias {alias}",
                plugins
                    .iter()
                    .map(|p| p.id().to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            return self;
        }

        let node = self.graph.add_node(alias.clone());
        self.aliases
            .extend(plugins.iter().map(|p| (p.id(), alias.clone())));
        self.node2entry.insert(node, PluginEntry::new(plugins));
        self.id2node.insert(alias.clone(), node);
        self.node2id.insert(node, alias);

        self
    }

    /// Update plugins or insert them. During update, the patched dependencies are untouched
    ///
    /// if the id is aliased, then the aliased entry will be updated. (see [`Self::add_plugins_as_alias`])
    pub fn upset_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        let plugins = plugins.into_boxed_vec();
        for plugin in plugins {
            let id = plugin.id();

            // Look for node with this id or look for aliases of this id
            if let Some(node) = self
                .id2node
                .get(&id)
                .or_else(|| self.aliases.get(&id).and_then(|id| self.id2node.get(id)))
            {
                let Some(entry) = self.node2entry.get_mut(node) else {
                    unreachable!()
                };
                if let Some(existing_plugin) = entry
                    .plugins
                    .iter_mut()
                    .find(|p| (&***p).type_id() == plugin.deref().type_id())
                {
                    *existing_plugin = plugin;
                } else {
                    log::error!(
                        "Tried to update Plugin id {id} (type: {}) with another \
                        plugin of a different type. (new type: {}). This is forbidden",
                        entry.plugins().first().unwrap().name(),
                        plugin.name()
                    );
                    continue;
                }
            } else {
                let node = self.graph.add_node(id.clone());
                self.node2entry.insert(node, PluginEntry::new(vec![plugin]));
                self.id2node.insert(id.clone(), node);
                self.node2id.insert(node, id);
            }
        }
        self
    }

    pub fn patch_plugin_dependencies(&mut self, patch: &PluginDependencyPatch) -> &mut Self {
        for (id, patch) in &patch.patches {
            self.update_plugin_dependencies(id, patch.iter().cloned());
        }
        self
    }

    fn update_plugin_dependencies(
        &mut self,
        id: &PluginId,
        patches: impl IntoIterator<Item = UpdatePluginDependency>,
    ) -> &mut Self {
        if let Some(n) = self.id2node.get(id) {
            let entry = self
                .node2entry
                .get_mut(n)
                .expect("A node entry must have the corresponding plugin entry");
            for patch in patches {
                match patch {
                    UpdatePluginDependency::AddBefore(id) => {
                        entry.add_before.insert(id);
                    }
                    UpdatePluginDependency::AddAfter(id) => {
                        entry.add_after.insert(id);
                    }
                    UpdatePluginDependency::RemoveBefore(id) => {
                        entry.remove_before.insert(id);
                    }
                    UpdatePluginDependency::RemoveAfter(id) => {
                        entry.remove_after.insert(id);
                    }
                }
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
        let Some(node) = self
            .id2node
            .get(id)
            .or_else(|| self.aliases.get(id).and_then(|id| self.id2node.get(id)))
        else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(entry) = self.node2entry.get(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = entry
            .plugins
            .iter()
            .find(|p| (&***p).type_id() == TypeId::of::<P>())
            .and_then(|p| p.downcast_ref::<P>())
        else {
            return Err(GetPluginError::InvalidType(
                id.clone(),
                core::any::type_name::<P>(),
            ));
        };
        Ok(plugin)
    }

    pub fn get_plugin_mut<P: Plugin>(&mut self, id: &PluginId) -> Result<&mut P, GetPluginError> {
        let Some(node) = self
            .id2node
            .get(id)
            .or_else(|| self.aliases.get(id).and_then(|id| self.id2node.get(id)))
        else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(entry) = self.node2entry.get_mut(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = entry
            .plugins
            .iter_mut()
            .find(|p| (&***p).type_id() == TypeId::of::<P>())
            .and_then(|p| p.downcast_mut::<P>())
        else {
            return Err(GetPluginError::InvalidType(
                id.clone(),
                core::any::type_name::<P>(),
            ));
        };
        Ok(plugin)
    }

    pub fn try_into_plugin_vec(self) -> Result<Vec<Box<dyn Plugin>>, PluginGraphBuildError> {
        let sorted = self.try_into_sorted_plugins()?;

        {
            use bevy_platform::prelude::*;
            log::trace!(
                "Sorted plugins:\n\n[\n{}\n]",
                sorted
                    .iter()
                    .flat_map(PluginEntry::plugins)
                    .map(|n| format!("\"{}\"", n.deref().id()))
                    .collect::<Vec<_>>()
                    .join(",\n")
            );
        }
        Ok(sorted.into_iter().flat_map(|entry| entry.plugins).collect())
    }

    pub fn iter_plugins(&self) -> impl Iterator<Item = &PluginEntry> {
        self.node2entry.values()
    }

    fn add_plugin_edges(mut self) -> Result<PluginGraph, PluginGraphBuildError> {
        // add dependencies (edges)
        for (&this_plugin, entry) in &self.node2entry {
            let plugin_id = self.node2id.get(&this_plugin).unwrap();
            for from in entry.build_after() {
                let (from, is_required) = match from {
                    PluginDependency::Required(id) => (id, true),
                    PluginDependency::Optional(id) => (id, false),
                };

                // id aliasing
                let from = self.aliases.get(&from).cloned().unwrap_or(from);
                // Skip self dependencies
                if &from == plugin_id {
                    continue;
                }

                if let Some(from) = self.id2node.get(&from) {
                    self.graph.add_edge(*from, this_plugin, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(from.clone()))?;
                } else {
                    log::warn!(
                        "Optional dependency not found: {plugin_id} should build after {from} (not found)"
                    );
                }
            }
            for to in entry.build_before() {
                let (to, is_required) = match to {
                    PluginDependency::Required(id) => (id, true),
                    PluginDependency::Optional(id) => (id, false),
                };

                // id aliasing
                let to = self.aliases.get(&to).cloned().unwrap_or(to);
                // Skip self dependencies
                if &to == plugin_id {
                    continue;
                }

                if let Some(to) = self.id2node.get(&to) {
                    self.graph.add_edge(this_plugin, *to, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(to.clone()))?;
                } else {
                    log::warn!(
                        "Optional dependency not found: {plugin_id} should build before {to} (not found)"
                    );
                }
            }
        }

        Ok(self)
    }

    fn try_into_sorted_plugins(self) -> Result<Vec<PluginEntry>, PluginGraphBuildError> {
        let mut value = self.add_plugin_edges()?;

        let sorted = petgraph::algo::toposort(&value.graph, None)
            .map_err(|err| {
                let cycle = find_cycle(err.node_id(), &value.graph);
                let cycle = cycle
                    .into_iter()
                    .map(|n| value.node2id.get(&n).cloned().unwrap())
                    .collect();
                PluginGraphBuildError::CircularDependency(cycle)
            })?
            .into_iter()
            .map(|n| value.node2entry.remove(&n).unwrap());

        Ok(sorted.collect())
    }
}

fn find_cycle(node_in_cycle: NodeIndex, graph: &Graph<PluginId, ()>) -> Vec<NodeIndex> {
    use bevy_platform::collections::*;
    use bevy_platform::prelude::*;
    use bevy_platform::sync::*;

    struct GraphNode {
        index: NodeIndex,
        parent: Option<Arc<RwLock<GraphNode>>>,
    }
    impl GraphNode {
        fn new(index: NodeIndex, parent: Option<Arc<RwLock<GraphNode>>>) -> Self {
            Self { index, parent }
        }

        fn add_child(this: &Arc<RwLock<GraphNode>>, child: NodeIndex) -> Arc<RwLock<GraphNode>> {
            Arc::new(RwLock::new(GraphNode::new(child, Some(this.clone()))))
        }
    }

    let root = Arc::new(RwLock::new(GraphNode::new(node_in_cycle, None)));
    let mut to_visit = Vec::new();
    to_visit.push(root);
    let mut visited = HashSet::new();

    while let Some(next) = to_visit.pop() {
        let current_node = next.read().unwrap_or_else(PoisonError::into_inner).index;
        for child in graph.edges_directed(current_node, Outgoing) {
            if child.target() == node_in_cycle {
                let mut nodes = vec![node_in_cycle, current_node];
                let mut current_node = next;
                while let Some(parent) = {
                    current_node
                        .read()
                        .unwrap_or_else(PoisonError::into_inner)
                        .parent
                        .clone()
                } {
                    current_node = parent;
                    nodes.push(
                        current_node
                            .read()
                            .unwrap_or_else(PoisonError::into_inner)
                            .index,
                    );
                }
                return nodes;
            }
            if !visited.contains(&child.target()) {
                visited.insert(child.target());
                let child = GraphNode::add_child(&next, child.target());
                to_visit.push(child);
            }
        }
    }
    vec![node_in_cycle]
}

#[derive(Debug, Error)]
pub enum PluginGraphBuildError {
    #[error("Missing required plugin: '{0}'")]
    MissingRequiredPlugin(PluginId),
    #[error("Circular dependency detected with plugin: '{}'", .0.iter().map(alloc::string::ToString::to_string).collect::<Vec<_>>().join(" -> ")
    )]
    CircularDependency(Vec<PluginId>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::Cow;
    use bevy_app_macros::plugin_deps_patch;

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
            let plugin_ids = plugins
                .iter()
                .flat_map(|e| e.plugins())
                .map(|p| p.deref().id())
                .collect::<Vec<_>>();

            assert_eq!(
                vec![
                    PluginId::of::<PluginA>(),
                    PluginId::of::<PluginB>(),
                    PluginId::of::<PluginC>()
                ],
                plugin_ids
            );
        }

        test((PluginA, PluginB, PluginC));
        test((PluginC, PluginA, PluginB));
        test((PluginB, PluginC, PluginA));
        test((PluginA, PluginC, PluginB));
        test((PluginB, PluginA, PluginC));
        test((PluginC, PluginB, PluginA));
    }

    #[test]
    fn build_order_with_patch() {
        fn test<M>(plugins: impl Plugins<M>) {
            let mut plugin_graph = PluginGraph::default();
            plugin_graph.add_plugins(plugins);

            plugin_graph.patch_plugin_dependencies(&plugin_deps_patch! {
                PluginA: {
                    build_before: [-PluginB],
                    build_after: [+PluginC],
                },
                PluginB: {
                    build_before: [+PluginC]
                },
                PluginC: {
                    build_after: [-PluginB]
                },
            });

            let plugins = plugin_graph.try_into_sorted_plugins().unwrap();
            let plugin_ids = plugins
                .iter()
                .flat_map(|e| e.plugins())
                .map(|p| p.deref().id())
                .collect::<Vec<_>>();

            assert_eq!(
                vec![
                    PluginId::of::<PluginB>(),
                    PluginId::of::<PluginC>(),
                    PluginId::of::<PluginA>()
                ],
                plugin_ids
            );
        }

        test((PluginA, PluginB, PluginC));
        test((PluginC, PluginA, PluginB));
        test((PluginB, PluginC, PluginA));
        test((PluginA, PluginC, PluginB));
        test((PluginB, PluginA, PluginC));
        test((PluginC, PluginB, PluginA));
    }
}
