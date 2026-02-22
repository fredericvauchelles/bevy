use bevy_app::app_builder::UpdatePluginDependency;
use bevy_app::PluginId;
use bevy_platform::collections::{HashMap, HashSet};

/// Patch plugin dependencies
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct PluginDependencyPatch {
    pub(crate) patches: HashMap<PluginId, HashSet<UpdatePluginDependency>>,
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