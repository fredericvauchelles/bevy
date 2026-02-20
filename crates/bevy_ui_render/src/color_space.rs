use alloc::borrow::Cow;
use bevy_app::{plugin_deps, App, Plugin, PluginDependency};
use bevy_render::RenderPlugin;
use bevy_shader::load_shader_library;

/// A plugin for WGSL color space utility functions
pub struct ColorSpacePlugin;

impl Plugin for ColorSpacePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "color_space.wgsl");
    }

    fn build_after(&'_ self) -> Cow<'_, [PluginDependency]> {
        plugin_deps!(RenderPlugin).into()
    }
}
