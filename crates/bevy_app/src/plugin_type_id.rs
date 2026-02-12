use crate::Plugin;
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use core::any::{type_name, TypeId};
use core::fmt::{Display, Formatter};
use core::hash::Hash;
use core::hash::Hasher;

/// Unique identifier of a plugin type
#[derive(Debug)]
pub struct PluginTypeId(TypeId, Cow<'static, str>);
impl Display for PluginTypeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.1)
    }
}
impl PartialEq for PluginTypeId {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl Eq for PluginTypeId {}
impl Hash for PluginTypeId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl PluginTypeId {
    /// Builds a [`PluginTypeId`] for specified plugin `P`
    pub fn of<P: Plugin>() -> Self {
        Self(TypeId::of::<P>(), Cow::Borrowed(type_name::<P>()))
    }

    pub fn new(plugin: &dyn Plugin) -> Self {
        Self(plugin.type_id(), Cow::Owned(plugin.name().to_owned()))
    }
}
