use scinumtools3_build::{Config, RustType};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

fn dip_sources(directory: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            dip_sources(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "dip")
            || path.file_name().is_some_and(|name| name == "DIPfile")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn format_dip_settings() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../settings");
    let mut files = Vec::new();
    dip_sources(&settings, &mut files)?;
    files.sort();
    println!("cargo:rerun-if-changed={}", settings.display());

    match Command::new("snt")
        .args(["dip", "format", "--help"])
        .output()
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(output) if !output.status.success() => {
            println!("cargo:warning=snt is installed without dip format; skipping DIPL formatting");
            return Ok(());
        }
        Ok(_) => {}
    }

    let mut check = Command::new("snt");
    check.args(["dip", "format", "--check"]).args(&files);
    match check.output() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(output) if output.status.success() => return Ok(()),
        Ok(_) => {}
    }

    let status = Command::new("snt")
        .args(["dip", "format", "--write"])
        .args(&files)
        .status()?;
    if !status.success() {
        return Err(io::Error::other("snt dip format --write failed").into());
    }
    Ok(())
}

#[cfg(unix)]
fn generate_settings_report() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../generate-settings-report.sh");
    let cheatsheet = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/dipl-cheatsheet.pdf");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", cheatsheet.display());

    match Command::new("sh").arg(&script).output() {
        Ok(output) if output.status.success() => {
            let message = String::from_utf8_lossy(&output.stdout);
            if !message.trim().is_empty() {
                println!("cargo:warning={}", message.trim());
            }
        }
        Ok(output) => {
            let message = String::from_utf8_lossy(&output.stderr);
            println!(
                "cargo:warning=Settings PDF generation failed: {}",
                message.trim()
            );
        }
        Err(error) => println!("cargo:warning=Settings PDF generation failed: {error}"),
    }
}

#[cfg(not(unix))]
fn generate_settings_report() {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    format_dip_settings()?;
    generate_settings_report();
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
