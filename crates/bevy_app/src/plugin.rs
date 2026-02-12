use crate::App;
use bevy_app::PluginTypeId;
use core::any::Any;
use downcast_rs::{impl_downcast, Downcast};

/// A collection of Bevy app logic and configuration.
///
/// Plugins configure an [`App`]. When an [`App`] registers a plugin,
/// the plugin's [`Plugin::build`] function is run. By default, a plugin
/// can only be added once to an [`App`].
///
/// If the plugin may need to be added twice or more, the function [`is_unique()`](Self::is_unique)
/// should be overridden to return `false`. Plugins are considered duplicate if they have the same
/// [`name()`](Self::name). The default `name()` implementation returns the type name, which means
/// generic plugins with different type parameters will not be considered duplicates.
///
/// ## Lifecycle of a plugin
///
/// When adding a plugin to an [`App`]:
/// * the app calls [`Plugin::build`] immediately, and register the plugin
/// * once the app started, it will wait for all registered [`Plugin::ready`] to return `true`
/// * it will then call all registered [`Plugin::finish`]
/// * and call all registered [`Plugin::cleanup`]
///
/// ## Defining a plugin.
///
/// Most plugins are simply functions that add configuration to an [`App`].
///
/// ```
/// # use bevy_app::{App, Update};
/// App::new().add_plugins(my_plugin).run();
///
/// // This function implements `Plugin`, along with every other `fn(&mut App)`.
/// pub fn my_plugin(app: &mut App) {
///     app.add_systems(Update, hello_world);
/// }
/// # fn hello_world() {}
/// ```
///
/// For more advanced use cases, the `Plugin` trait can be implemented manually for a type.
///
/// ```
/// # use bevy_app::*;
/// pub struct AccessibilityPlugin {
///     pub flicker_damping: bool,
///     // ...
/// }
///
/// impl Plugin for AccessibilityPlugin {
///     fn build(&self, app: &mut App) {
///         if self.flicker_damping {
///             app.add_systems(PostUpdate, damp_flickering);
///         }
///     }
/// }
/// # fn damp_flickering() {}
/// ```
pub trait Plugin: Downcast + Any + Send + Sync {
    /// Configures the [`App`] to which this plugin is added.
    fn build(&self, app: &mut App);

    /// Has the plugin finished its setup? This can be useful for plugins that need something
    /// asynchronous to happen before they can finish their setup, like the initialization of a renderer.
    /// Once the plugin is ready, [`finish`](Plugin::finish) should be called.
    fn ready(&self, _app: &App) -> bool {
        true
    }

    /// Finish adding this plugin to the [`App`], once all plugins registered are ready. This can
    /// be useful for plugins that depends on another plugin asynchronous setup, like the renderer.
    fn finish(&self, _app: &mut App) {
        // do nothing
    }

    /// Runs after all plugins are built and finished, but before the app schedule is executed.
    /// This can be useful if you have some resource that other plugins need during their build step,
    /// but after build you want to remove it and send it to another thread.
    fn cleanup(&self, _app: &mut App) {
        // do nothing
    }

    /// Configures a name for the [`Plugin`] which is primarily used for checking plugin
    /// uniqueness and debugging.
    fn name(&self) -> &str {
        core::any::type_name::<Self>()
    }

    /// If the plugin can be meaningfully instantiated several times in an [`App`],
    /// override this method to return `false`.
    fn is_unique(&self) -> bool {
        true
    }

    /// This plugin assumes that these plugins are added before this one
    fn depends_on(&self) -> alloc::vec::Vec<PluginTypeId> {
        alloc::vec::Vec::new()
    }
}

impl_downcast!(Plugin);

impl<T: Fn(&mut App) + Send + Sync + 'static> Plugin for T {
    fn build(&self, app: &mut App) {
        self(app);
    }
}

/// Plugins state in the application
#[derive(PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum PluginsState {
    /// Plugins are being added.
    Adding,
    /// All plugins already added are ready.
    Ready,
    /// Finish has been executed for all plugins added.
    Finished,
    /// Cleanup has been executed for all plugins added.
    Cleaned,
}

/// A dummy plugin that's to temporarily occupy an entry in an app's plugin registry.
pub(crate) struct PlaceholderPlugin;

impl Plugin for PlaceholderPlugin {
    fn build(&self, _app: &mut App) {}
}

/// Types that represent a set of [`Plugin`]s.
///
/// This is implemented for all types which implement [`Plugin`],
/// [`PluginGroup`](super::PluginGroup), and tuples over [`Plugins`].
pub trait Plugins<Marker>: sealed::Plugins<Marker> {
    fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId>;
    fn into_boxed_vec(self) -> alloc::vec::Vec<alloc::boxed::Box<dyn Plugin>>;
}

impl<Marker, T> Plugins<Marker> for T
where
    T: sealed::Plugins<Marker>,
{
    fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId> {
        <Self as sealed::Plugins<Marker>>::plugin_type_ids(self)
    }
    fn into_boxed_vec(self) -> alloc::vec::Vec<alloc::boxed::Box<dyn Plugin>> {
        <Self as sealed::Plugins<Marker>>::into_boxed_vec(self)
    }
}

mod sealed {
    use crate::{App, AppError, Plugin, PluginGroup};
    use alloc::boxed::Box;
    use bevy_app::PluginTypeId;
    use variadics_please::all_tuples;

    pub trait Plugins<Marker> {
        fn add_to_app(self, app: &mut App);
        fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId>;
        fn into_boxed_vec(self) -> alloc::vec::Vec<Box<dyn Plugin>>;
    }

    pub struct PluginMarker;
    pub struct PluginGroupMarker;
    pub struct PluginsTupleMarker;

    impl<P: Plugin> Plugins<PluginMarker> for P {
        #[track_caller]
        fn add_to_app(self, app: &mut App) {
            if let Err(AppError::DuplicatePlugin { plugin_name }) =
                app.add_boxed_plugin(Box::new(self))
            {
                panic!(
                    "Error adding plugin {plugin_name}: : plugin was already added in application"
                )
            }
        }
        fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId> {
            alloc::vec![PluginTypeId::of::<P>()]
        }
        fn into_boxed_vec(self) -> alloc::vec::Vec<Box<dyn Plugin>> {
            alloc::vec![Box::new(self)]
        }
    }

    impl<P: PluginGroup + Clone> Plugins<PluginGroupMarker> for P {
        #[track_caller]
        fn add_to_app(self, app: &mut App) {
            self.build().finish(app);
        }

        fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId> {
            self.clone()
                .into_boxed_vec()
                .into_iter()
                .map(|p| PluginTypeId::new(&*p))
                .collect()
        }
        fn into_boxed_vec(self) -> alloc::vec::Vec<Box<dyn Plugin>> {
            self.build().into()
        }
    }

    macro_rules! impl_plugins_tuples {
        ($(#[$meta:meta])* $(($param: ident, $plugins: ident, $plugins_lo: ident)),*) => {
            $(#[$meta])*
            impl<$($param, $plugins),*> Plugins<(PluginsTupleMarker, $($param,)*)> for ($($plugins,)*)
            where
                $($plugins: Plugins<$param>),*
            {
                #[expect(
                    clippy::allow_attributes,
                    reason = "This is inside a macro, and as such, may not trigger in all cases."
                )]
                #[allow(non_snake_case, reason = "`all_tuples!()` generates non-snake-case variable names.")]
                #[allow(unused_variables, reason = "`app` is unused when implemented for the unit type `()`.")]
                #[track_caller]
                fn add_to_app(self, app: &mut App) {
                    let ($($plugins_lo,)*) = self;
                    $($plugins_lo.add_to_app(app);)*
                }

                fn plugin_type_ids(&self) -> alloc::vec::Vec<PluginTypeId> {
                    let ($($plugins_lo,)*) = self;
                    let values = core::iter::empty();
                    $(let values = values.chain($plugins_lo.plugin_type_ids().into_iter());)*
                    values.collect()
                }
                fn into_boxed_vec(self) -> alloc::vec::Vec<Box<dyn Plugin>> {
                    let ($($plugins_lo,)*) = self;
                    let values = core::iter::empty();
                    $(let values = values.chain($plugins_lo.into_boxed_vec().into_iter());)*
                    values.collect()
                }
            }
        }
    }

    all_tuples!(
        #[doc(fake_variadic)]
        impl_plugins_tuples,
        0,
        15,
        P,
        S,
        s
    );
}

#[cfg(test)]
mod tests {
    use super::{Plugin, PluginTypeId, Plugins};
    use bevy_app::App;
    use std::prelude::rust_2015::Vec;
    use std::vec;

    #[derive(Default)]
    struct EmptyPlugin2;
    impl Plugin for EmptyPlugin2 {
        fn build(&self, _app: &mut App) {}

        fn depends_on(&self) -> Vec<PluginTypeId> {
            vec![PluginTypeId::of::<EmptyPlugin3>()]
        }
    }

    #[derive(Default)]
    struct EmptyPlugin3;
    impl Plugin for EmptyPlugin3 {
        fn build(&self, _app: &mut App) {}
    }

    #[test]
    fn plugin_type_ids() {
        assert_eq!(
            vec![
                PluginTypeId::of::<EmptyPlugin2>(),
                PluginTypeId::of::<EmptyPlugin3>()
            ],
            (EmptyPlugin2::default(), EmptyPlugin3::default()).plugin_type_ids()
        );
    }
}
