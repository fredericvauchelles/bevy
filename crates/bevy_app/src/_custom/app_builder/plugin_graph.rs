//! [`PluginGraph`] is a graph of plugin dependencies, required to build an appropriate order of
//! plugin execution

use alloc::boxed::Box;
use bevy_app::*;
use bevy_ecs::prelude::*;
use bevy_platform::collections::*;
use bevy_platform::prelude::*;
use core::ops::{Deref, DerefMut};
use petgraph::prelude::*;
use thiserror::Error;

/// Patch plugin dependencies
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct PluginDependencyPatch {
    patches: HashMap<PluginId, HashSet<UpdatePluginDependency>>,
}
impl PluginDependencyPatch {
    /// Creates a new [`PluginDependencyPatch`]
    pub fn new(patches: impl IntoIterator<Item=(PluginId, HashSet<UpdatePluginDependency>)>) -> Self {
        Self {
            patches: patches.into_iter().collect()
        }
    }

    /// Adds a dependency
    pub fn add(&mut self, id: PluginId, dep: UpdatePluginDependency) {
        self.patches.entry(id).or_default().insert(dep);
    }

    /// Removes a dependency
    pub fn remove(&mut self, id: &PluginId, dep: &UpdatePluginDependency) {
        self.patches.get_mut(id).map(|entry| entry.remove(dep));
    }
}
impl Extend<(PluginId, UpdatePluginDependency)> for PluginDependencyPatch {
    fn extend<T: IntoIterator<Item=(PluginId, UpdatePluginDependency)>>(&mut self, iter: T) {
        for (id, dep) in iter {
            self.add(id, dep);
        }
    }
}
impl Extend<(PluginId, HashSet<UpdatePluginDependency>)> for PluginDependencyPatch {
    fn extend<T: IntoIterator<Item=(PluginId, HashSet<UpdatePluginDependency>)>>(&mut self, iter: T) {
        for (id, dep) in iter {
            self.patches.entry(id).or_default().extend(dep)
        }
    }
}
impl FromIterator<(PluginId, UpdatePluginDependency)> for PluginDependencyPatch {
    fn from_iter<T: IntoIterator<Item=(PluginId, UpdatePluginDependency)>>(iter: T) -> Self {
        let mut val = Self::default();
        val.extend(iter);
        val
    }
}
impl FromIterator<(PluginId, HashSet<UpdatePluginDependency>)> for PluginDependencyPatch {
    fn from_iter<T: IntoIterator<Item=(PluginId, HashSet<UpdatePluginDependency>)>>(iter: T) -> Self {
        let mut val = Self::default();
        val.extend(iter);
        val
    }
}

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

pub struct PluginEntry {
    plugin: Box<dyn Plugin>,
    add_before: HashSet<PluginDependency>,
    add_after: HashSet<PluginDependency>,
    remove_before: HashSet<PluginDependency>,
    remove_after: HashSet<PluginDependency>,
}
impl PluginEntry {
    pub fn plugin(&self) -> &dyn Plugin {
        self.plugin.deref()
    }
    pub fn build_before(&self) -> impl IntoIterator<Item=PluginDependency> {
        self.plugin().build_before().iter()
            .filter(|id| !self.remove_before.contains(*id))
            .chain(self.add_before.iter())
            .cloned()
            .collect::<HashSet<_>>()
    }

    pub fn build_after(&self) -> impl IntoIterator<Item=PluginDependency> {
        self.plugin().build_after().iter()
            .filter(|id| !self.remove_after.contains(*id))
            .chain(self.add_after.iter())
            .cloned()
            .collect::<HashSet<_>>()
    }

    fn new(plugin: Box<dyn Plugin>) -> Self {
        Self {
            plugin,
            remove_before: Default::default(),
            remove_after: Default::default(),
            add_before: Default::default(),
            add_after: Default::default(),
        }
    }
}
impl From<PluginEntry> for Box<dyn Plugin> {
    fn from(value: PluginEntry) -> Self {
        value.plugin
    }
}

#[derive(Default)]
pub struct PluginGraph {
    plugins: HashMap<NodeIndex, PluginEntry>,
    id2node: HashMap<PluginId, NodeIndex>,
    graph: Graph<PluginId, ()>,
}

/// Describe how to update a plugin dependency
#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum UpdatePluginDependency {
    /// Add dependency in build_before
    AddBefore(PluginDependency),
    /// Add dependency in build_after
    AddAfter(PluginDependency),
    /// Remove dependency in build_before
    RemoveBefore(PluginDependency),
    /// Remove dependency in build_after
    RemoveAfter(PluginDependency),
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
            self.plugins.insert(node, PluginEntry::new(plugin));
            self.id2node.insert(id, node);
        }
        self
    }

    /// Update plugins or insert them. During update, the patched dependencies are untouched
    pub fn upset_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        let plugins = plugins.into_boxed_vec();
        for plugin in plugins {
            let id = plugin.id();
            if let Some(node) = self.id2node.get(&id) {
                let Some(entry) = self.plugins.get_mut(node) else {
                    unreachable!()
                };
                if entry.plugin().type_id() != plugin.deref().type_id() {
                    log::error!("Tried to update Plugin id {id} (type: {}) with another \
                        plugin of a different type. (new type: {}). This is forbidden",
                        entry.plugin().name(), plugin.name());
                    continue
                }
                entry.plugin = plugin;
            } else {
                let node = self.graph.add_node(id.clone());
                self.plugins.insert(node, PluginEntry::new(plugin));
                self.id2node.insert(id, node);
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

    fn update_plugin_dependencies(&mut self, id: &PluginId, patches: impl IntoIterator<Item=UpdatePluginDependency>) -> &mut Self {
        if let Some(n) = self.id2node.get(id) {
            let entry = self.plugins.get_mut(n).expect("A node entry must have the corresponding plugin entry");
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
        let Some(node) = self.id2node.get(id) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(entry) = self.plugins.get(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = entry.plugin().downcast_ref::<P>() else {
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
        let Some(entry) = self.plugins.get_mut(node) else {
            return Err(GetPluginError::PluginNotAdded(id.clone()));
        };
        let Some(plugin) = entry.plugin.deref_mut().downcast_mut::<P>() else {
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

    pub fn iter_plugins(&self) -> impl Iterator<Item=&PluginEntry> {
        self.plugins.values()
    }

    fn add_plugin_edges(mut self) -> Result<PluginGraph, PluginGraphBuildError> {
        // add dependencies (edges)
        for entry in self.plugins.values() {
            let plugin_id = entry.plugin().id();
            let this_plugin = *self.id2node.get(&plugin_id).unwrap();
            for from in entry.build_after() {
                let (from, is_required) = match from {
                    PluginDependency::Required(id) => (id, true),
                    PluginDependency::Optional(id) => (id, false),
                };
                if let Some(from) = self.id2node.get(&from) {
                    self.graph.add_edge(*from, this_plugin, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(from.clone()))?
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
                if let Some(to) = self.id2node.get(&to) {
                    self.graph.add_edge(this_plugin, *to, ());
                } else if is_required {
                    Err(PluginGraphBuildError::MissingRequiredPlugin(to.clone()))?
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

        let sorted = petgraph::algo::toposort(&value.graph, None).map_err(|err| {
            let cycle = find_cycle(err.node_id(), &value.graph);
            let cycle = cycle.into_iter().map(|n| value.plugins.get(&n).unwrap().plugin.id()).collect();
            PluginGraphBuildError::CircularDependency(
                cycle
            )
        })?
            .into_iter()
            .map(|n| value.plugins.remove(&n).unwrap());

        Ok(sorted.collect())
    }
}

fn find_cycle(node_in_cycle: NodeIndex, graph: &Graph<PluginId, ()>) -> Vec<NodeIndex> {
    use bevy_platform::prelude::*;
    use bevy_platform::collections::*;
    use bevy_platform::sync::*;

    struct GraphNode {
        index: NodeIndex,
        parent: Option<Arc<RwLock<GraphNode>>>,
    }
    impl GraphNode {
        fn new(index: NodeIndex, parent: Option<Arc<RwLock<GraphNode>>>) -> Self {
            Self {
                index,
                parent,
            }
        }

        fn add_child(this: &Arc<RwLock<GraphNode>>, child: NodeIndex) -> Arc<RwLock<GraphNode>> {
            let child = Arc::new(RwLock::new(GraphNode::new(child, Some(this.clone()))));
            child
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
                while let Some(parent) = { current_node.read().unwrap_or_else(PoisonError::into_inner).parent.clone() } {
                    current_node = parent;
                    nodes.push(current_node.read().unwrap_or_else(PoisonError::into_inner).index);
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
    #[error("Circular dependency detected with plugin: '{}'", .0.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" -> ")
    )]
    CircularDependency(Vec<PluginId>),
}

impl TryFrom<PluginGraph> for PluginGroupBuilder {
    type Error = BevyError;

    fn try_from(value: PluginGraph) -> Result<Self, Self::Error> {
        let sorted = value.try_into_sorted_plugins()?;

        {
            use bevy_platform::prelude::*;
            log::trace!("Sorted plugins:\n\n[\n{}\n]", sorted.iter().map(|n| format!("\"{}\"", n.plugin().id())).collect::<Vec<_>>().join(",\n"));
        }
        let mut result = PluginGraphPluginGroup.build();
        result.extend(sorted.into_iter().map(Into::into));
        Ok(result)
    }
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
            let plugin_ids = plugins.iter().map(|p| p.plugin().id()).collect::<Vec<_>>();

            assert_eq!(vec![PluginId::of::<PluginA>(), PluginId::of::<PluginB>(), PluginId::of::<PluginC>()], plugin_ids);
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

            plugin_graph.patch_plugin_dependencies(
                &plugin_deps_patch! {
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
                }
            );


            let plugins = plugin_graph.try_into_sorted_plugins().unwrap();
            let plugin_ids = plugins.iter().map(|p| p.plugin().id()).collect::<Vec<_>>();

            assert_eq!(vec![PluginId::of::<PluginB>(), PluginId::of::<PluginC>(), PluginId::of::<PluginA>()], plugin_ids);
        }

        test((PluginA, PluginB, PluginC));
        test((PluginC, PluginA, PluginB));
        test((PluginB, PluginC, PluginA));
        test((PluginA, PluginC, PluginB));
        test((PluginB, PluginA, PluginC));
        test((PluginC, PluginB, PluginA));
    }
}