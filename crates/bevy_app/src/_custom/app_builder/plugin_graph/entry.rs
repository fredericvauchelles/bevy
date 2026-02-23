use bevy_app::{Plugin, PluginDependency, PluginId};
use bevy_platform::collections::HashSet;
use std::prelude::rust_2015::{Box, Vec};
use thiserror::Error;

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
    pub(super) plugins: Vec<Box<dyn Plugin>>,
    pub(super) add_before: HashSet<PluginDependency>,
    pub(super) add_after: HashSet<PluginDependency>,
    pub(super) remove_before: HashSet<PluginDependency>,
    pub(super) remove_after: HashSet<PluginDependency>,
}

impl PluginEntry {
    pub fn plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }
    pub fn build_before(&self) -> impl IntoIterator<Item=PluginDependency> {
        self.plugins().iter().map(|p| p.build_before())
            .flat_map(|p| p.into_owned().into_iter())
            .filter(|id| !self.remove_before.contains(id))
            .chain(self.add_before.clone().into_iter())
            .collect::<HashSet<_>>()
    }

    pub fn build_after(&self) -> impl IntoIterator<Item=PluginDependency> {
        self.plugins().iter().map(|p| p.build_after())
            .flat_map(|p| p.into_owned().into_iter())
            .filter(|id| !self.remove_after.contains(id))
            .chain(self.add_after.clone().into_iter())
            .collect::<HashSet<_>>()
    }

    pub(crate) fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
        Self {
            plugins,
            remove_before: Default::default(),
            remove_after: Default::default(),
            add_before: Default::default(),
            add_after: Default::default(),
        }
    }
}

impl From<PluginEntry> for Vec<Box<dyn Plugin>> {
    fn from(value: PluginEntry) -> Self {
        value.plugins
    }
}