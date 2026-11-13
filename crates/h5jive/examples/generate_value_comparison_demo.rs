//! Small, exact numeric arrays for the value comparison plugin.
use hdf5_metno::{File, Group};
use std::{error::Error, path::Path};

fn array(group: &Group, name: &str, shape: &[usize], values: &[f64]) -> Result<(), Box<dyn Error>> {
    group
        .new_dataset::<f64>()
        .shape(shape)
        .create(name)?
        .write_raw(values)?;
    Ok(())
}

fn write(path: &Path, is_b: bool) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let field = file.create_group("field")?;
    let plane = (1..=12).map(f64::from).collect::<Vec<_>>();
    let cube = (1..=24).map(f64::from).collect::<Vec<_>>();
    array(
        &field,
        "plane",
        &[3, 4],
        &plane
            .iter()
            .map(|&value| if is_b { value + 2.0 } else { value })
            .collect::<Vec<_>>(),
    )?;
    array(
        &field,
        "reference",
        &[3, 4],
        &plane
            .iter()
            .map(|&value| if is_b { value * 2.0 } else { value + 1.0 })
            .collect::<Vec<_>>(),
    )?;
    array(
        &field,
        "zero_reference",
        &[3, 4],
        &plane
            .iter()
            .enumerate()
            .map(|(index, &value)| if is_b && index % 4 == 0 { 0.0 } else { value })
            .collect::<Vec<_>>(),
    )?;
    array(
        &field,
        "cube",
        &[2, 3, 4],
        &cube
            .iter()
            .map(|&value| if is_b { value + 3.0 } else { value })
            .collect::<Vec<_>>(),
    )?;
    array(
        &field,
        "other_shape",
        if is_b { &[2, 4] } else { &[2, 3] },
        if is_b { &[1.0; 8] } else { &[1.0; 6] },
    )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = Path::new("examples/plugins/value_comparison");
    std::fs::create_dir_all(output)?;
    for (name, is_b) in [("value_demo_a.hdf5", false), ("value_demo_b.hdf5", true)] {
        let path = output.join(name);
        let pending = path.with_extension("hdf5.new");
        write(&pending, is_b)?;
        std::fs::rename(pending, path)?;
    }
    println!("Generated examples/plugins/value_comparison/value_demo_a.hdf5 and value_demo_b.hdf5");
    Ok(())
}
