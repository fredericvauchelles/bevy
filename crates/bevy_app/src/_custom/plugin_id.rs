use crate::Plugin;
use alloc::borrow::Cow;
use alloc::string::ToString;
use core::any::type_name;
use core::fmt::{Display, Formatter};
use core::hash::Hash;
use uuid::Uuid;

/// Unique identifier of a plugin type
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct PluginId(Cow<'static, str>);
impl Display for PluginId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl PluginId {
    /// Builds a [`PluginId`] for the specified plugin `P` using its [`core::any::type_name`].
    pub fn of<P: Plugin + ?Sized>() -> Self {
        Self(Cow::Borrowed(type_name::<P>()))
    }

    /// Builds a random [`PluginId`]
    pub fn random() -> Self {
        Self(Cow::Owned(Uuid::new_v4().to_string()))
    }
}

impl<P: Into<Cow<'static, str>>> From<P> for PluginId {
    fn from(value: P) -> Self {
        Self(value.into())
    }
}

/// Generates a `Cow<'static, [PluginDependency]>` of [`crate::prelude::PluginDependency`] with provided plugins.
///
/// You can either use the type or a string literal of the type_name
/// Prefer to use the type when possible, the string literal is helpful to avoid
/// importing the said type
///
/// Use `?` before the plugin to make the dependency optional
///
/// ```
/// # mod bevy_asset {
/// #     pub struct AssetPlugin;
/// # }
/// plugin_ids!(bevy_asset::AssetPlugin, "bevy_render::RenderPlugin", ?ImagePlugin, ?"my::other::plugin")
/// ```
#[macro_export]
macro_rules! plugin_deps {
    ([$(,)? $(#[$attr:meta])* $ty:path] -> [$($body:tt)*]) => {
        plugin_deps!([] -> [$(#[$attr])* $crate::prelude::PluginDependency::Required($crate::prelude::PluginId::of::<$ty>()), $($body)*])
    };
    ([$(,)? $(#[$attr:meta])* $ty:path, $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_deps!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginDependency::Required($crate::prelude::PluginId::of::<$ty>()), $($body)*])
    };
    ([$(,)? $(#[$attr:meta])* $type_name:literal $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_deps!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginDependency::Required($crate::prelude::PluginId::from($type_name)), $($body)*])
    };
    ([$(,)? $(#[$attr:meta])* ?$ty:path] -> [$($body:tt)*]) => {
        plugin_deps!([] -> [$(#[$attr])* $crate::prelude::PluginDependency::Optional($crate::prelude::PluginId::of::<$ty>()), $($body)*])
    };
    ([$(,)? $(#[$attr:meta])* ?$ty:path, $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_deps!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginDependency::Optional($crate::prelude::PluginId::of::<$ty>()), $($body)*])
    };
    ([$(,)? $(#[$attr:meta])* ?$type_name:literal $($tt:tt)*] -> [$($body:tt)*]) => {
        plugin_deps!([$($tt)*] -> [$(#[$attr])* $crate::prelude::PluginDependency::Optional($crate::prelude::PluginId::from($type_name)), $($body)*])
    };
    ([$(,)? ] -> [$($body:tt)*]) => {
        alloc::vec![$($body)*].into()
    };
    ($($tt:tt)*) => {
        plugin_deps!{[$($tt)+] -> []}
    };
}
