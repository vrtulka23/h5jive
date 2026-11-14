use scinumtools3_build::{Config, RustType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Config::new("../../settings/build/DIPfile")
        .rust_struct("crate::app::plugins::PluginLayout")
        .const_name("PLUGIN_LAYOUT")
        .field("rail_width", "plugin_layout.rail_width", RustType::F32)
        .field(
            "button_height",
            "plugin_layout.button_height",
            RustType::F32,
        )
        .generate("compiled_settings.rs")?;
    Config::new("../../settings/build/DIPfile")
        .rust_struct("crate::app::settings::ResourceLimits")
        .const_name("RESOURCE_LIMITS")
        .field(
            "tree_max_depth",
            "resource_limits.tree_max_depth",
            RustType::Usize,
        )
        .field(
            "tree_max_objects",
            "resource_limits.tree_max_objects",
            RustType::Usize,
        )
        .field(
            "statistics_sample_values",
            "resource_limits.statistics_sample_values",
            RustType::Usize,
        )
        .field(
            "slice_max_cells",
            "resource_limits.slice_max_cells",
            RustType::Usize,
        )
        .field(
            "volume_preview_max_voxels",
            "resource_limits.volume_preview_max_voxels",
            RustType::Usize,
        )
        .generate("compiled_limits.rs")?;
    Ok(())
}
