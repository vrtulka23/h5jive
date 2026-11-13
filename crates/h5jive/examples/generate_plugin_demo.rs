//! Reproducible A/B HDF5 fixtures for the Plugin Lab comparison.
use hdf5_metno::{types::VarLenUnicode, Dataset, File, Group};
use std::{error::Error, path::Path, str::FromStr};

fn dataset_text_attr(dataset: &Dataset, name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    dataset
        .new_attr::<VarLenUnicode>()
        .create(name)?
        .write_scalar(&VarLenUnicode::from_str(value)?)?;
    Ok(())
}

fn group_text_attr(group: &Group, name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    group
        .new_attr::<VarLenUnicode>()
        .create(name)?
        .write_scalar(&VarLenUnicode::from_str(value)?)?;
    Ok(())
}

fn numbers(
    group: &Group,
    name: &str,
    values: &[f64],
    units: &str,
) -> Result<Dataset, Box<dyn Error>> {
    let dataset = group.new_dataset_builder().with_data(values).create(name)?;
    dataset_text_attr(&dataset, "units", units)?;
    Ok(dataset)
}

fn write(path: &Path, is_b: bool) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let measurements = file.create_group("measurements")?;
    group_text_attr(
        &measurements,
        "processing",
        if is_b { "reprocessed" } else { "baseline" },
    )?;

    let shared = numbers(&measurements, "shared", &[1.0, 2.0, 3.0], "K")?;
    dataset_text_attr(&shared, "instrument", "H5Jive demo")?;
    numbers(
        &measurements,
        "changed_shape",
        if is_b {
            &[1.0, 2.0, 3.0, 4.0]
        } else {
            &[1.0, 2.0]
        },
        "K",
    )?;
    if is_b {
        let dataset = measurements
            .new_dataset_builder()
            .with_data(&[1_i32, 2, 3])
            .create("changed_datatype")?;
        dataset_text_attr(&dataset, "units", "count")?;
    } else {
        let dataset = measurements
            .new_dataset_builder()
            .with_data(&[1_f32, 2.0, 3.0])
            .create("changed_datatype")?;
        dataset_text_attr(&dataset, "units", "count")?;
    }
    numbers(
        &measurements,
        "changed_attribute",
        &[273.0, 274.0],
        if is_b { "C" } else { "K" },
    )?;
    let missing_attr = numbers(&measurements, "missing_attribute", &[5.0, 6.0], "K")?;
    if !is_b {
        dataset_text_attr(&missing_attr, "calibration", "version 1")?;
    }
    // Equal metadata with unequal values demonstrates that values are not compared.
    numbers(
        &measurements,
        "value_only",
        if is_b { &[8.0, 9.0] } else { &[1.0, 2.0] },
        "K",
    )?;
    numbers(
        &measurements,
        if is_b { "only_b" } else { "only_a" },
        &[42.0],
        "K",
    )?;

    let coordinates = file.create_group("coordinates")?;
    group_text_attr(&coordinates, "crs", "EPSG:4326")?;
    numbers(
        &coordinates,
        "latitude",
        &[48.0, 49.0, 50.0],
        "degrees_north",
    )?;
    numbers(
        &coordinates,
        "longitude",
        &[10.0, 11.0, 12.0],
        "degrees_east",
    )?;

    let quality = file.create_group("quality")?;
    group_text_attr(&quality, "meaning", "0=good, 1=bad")?;
    quality
        .new_dataset_builder()
        .with_data(&[0_u8, 0, 1])
        .create("flags")?;

    if is_b {
        numbers(&file.create_group("extra_b")?, "signal", &[2.0, 3.0], "K")?;
        numbers(&file, "structure_variant", &[1.0, 2.0], "K")?;
    } else {
        numbers(&file.create_group("extra_a")?, "signal", &[1.0, 2.0], "K")?;
        let group = file.create_group("structure_variant")?;
        group_text_attr(&group, "note", "This path is a group only in A")?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = Path::new("examples/plugins/path_comparison");
    std::fs::create_dir_all(output)?;
    for (name, is_b) in [("plugin_demo_a.hdf5", false), ("plugin_demo_b.hdf5", true)] {
        let path = output.join(name);
        let pending = path.with_extension("hdf5.new");
        write(&pending, is_b)?;
        std::fs::rename(pending, path)?;
    }
    println!("Generated examples/plugins/path_comparison/plugin_demo_a.hdf5 and examples/plugins/path_comparison/plugin_demo_b.hdf5");
    Ok(())
}
