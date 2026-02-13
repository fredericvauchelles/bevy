use crate::Plugin;
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use core::any::type_name;
use core::fmt::{Display, Formatter};
use core::hash::Hash;

/// Unique identifier of a plugin type
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct PluginTypeId(Cow<'static, str>);
impl Display for PluginTypeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl PluginTypeId {
    /// Builds a [`PluginTypeId`] for specified plugin `P`
    pub fn of<P: Plugin>() -> Self {
        Self(Cow::Borrowed(type_name::<P>()))
    }

    /// Builds a [`PluginTypeId`] for a [`Plugin`] dyn object
    pub fn new(plugin: &dyn Plugin) -> Self {
        Self(Cow::Owned(plugin.name().to_owned()))
    }

    /// Builds a [`PluginTypeId`] from a static str
    ///
    /// This must be [`core::any::type_name::<P>()`] value of the plugin
    pub fn from_static_str(type_name: &'static str) -> Self {
        Self(Cow::Borrowed(type_name))
    }
}

/// Generates a vec of the provided plugin types.
///
/// You can either use the type or a string literal of the type_name
/// Prefer to use the type when possible, the string literal is helpful to avoid
/// importing the said type
///
/// ```
/// # mod bevy_asset {
/// #     pub struct AssetPlugin;
/// # }
/// plugin_type_ids_of!(bevy_asset::AssetPlugin, "bevy_render::RenderPlugin")
/// ```
#[macro_export]
macro_rules! plugin_type_ids_of {
    ([$(,)? $(#[$attr:meta])* $ty:path] -> [$($body:tt)*]) => {
        plugin_type_ids_of!([] -> [$(#[$attr])* $crate::prelude::PluginTypeId::of::<$ty>(), $($body)*]);
    };
    ([$(,)? $(#[$attr:meta])* $ty:path, $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_type_ids_of!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginTypeId::of::<$ty>(), $($body)*]);
    };
    ([$(,)? $(#[$attr:meta])* $type_name:literal $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_type_ids_of!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginTypeId::from_static_str($type_name), $($body)*]);
    };
    ([$(,)? ] -> [$($body:tt)*]) => {
        vec![$($body)*]
    };
    ($($tt:tt)*) => {
        plugin_type_ids_of!{[$($tt)+] -> []}
    };
}
