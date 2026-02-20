//! Additional code to the bevy project

pub mod app_builder;
pub mod plugin_id;

pub mod prelude {
    pub use super::app_builder::prelude::*;
    pub use super::plugin_id::*;
    pub use bevy_app_macros::plugin_deps_patch;
}
