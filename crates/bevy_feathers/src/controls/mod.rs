//! Meta-module containing all feathers controls (widgets that are interactive).

use bevy_app::app_builder::AppBuilder;
use bevy_app::Plugin;

mod button;
mod checkbox;
mod color_plane;
mod color_slider;
mod color_swatch;
mod radio;
mod slider;
mod toggle_switch;
mod virtual_keyboard;

pub use button::{button, ButtonPlugin, ButtonProps, ButtonVariant};
pub use checkbox::{checkbox, CheckboxPlugin};
pub use color_plane::{color_plane, ColorPlane, ColorPlaneValue};
pub use color_slider::{
    color_slider, ColorChannel, ColorSlider, ColorSliderPlugin, ColorSliderProps, SliderBaseColor,
};
pub use color_swatch::{color_swatch, ColorSwatch, ColorSwatchFg, ColorSwatchValue};
pub use radio::{radio, RadioPlugin};
pub use slider::{slider, SliderPlugin, SliderProps};
pub use toggle_switch::{toggle_switch, ToggleSwitchPlugin};
pub use virtual_keyboard::{virtual_keyboard, VirtualKeyPressed};

use crate::{
    alpha_pattern::AlphaPatternPlugin,
    controls::{color_plane::ColorPlanePlugin, color_swatch::ColorSwatchPlugin},
};

/// Plugin which registers all `bevy_feathers` controls.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, _: &mut bevy_app::App) {}
    fn pre_build(&self) -> Option<Box<dyn FnOnce(&mut AppBuilder)>> {
        Some(Box::new(|app| {
            app.add_plugins((
                AlphaPatternPlugin,
                ButtonPlugin,
                CheckboxPlugin,
                ColorPlanePlugin,
                ColorSliderPlugin,
                ColorSwatchPlugin,
                RadioPlugin,
                SliderPlugin,
                ToggleSwitchPlugin,
            ));
        }))
    }
}
