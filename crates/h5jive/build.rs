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
    Ok(())
}
