//! Deterministic smooth fields for the value comparison plugin.
use hdf5_metno::{Dataset, File, Group};
use std::{error::Error, path::Path};

const FILL_VALUE: f64 = -9999.0;

fn array(
    group: &Group,
    name: &str,
    shape: &[usize],
    values: &[f64],
) -> Result<Dataset, Box<dyn Error>> {
    let dataset = group.new_dataset::<f64>().shape(shape).create(name)?;
    dataset.write_raw(values)?;
    Ok(dataset)
}

fn gaussian(x: f64, y: f64, center_x: f64, center_y: f64, width_x: f64, width_y: f64) -> f64 {
    let dx = (x - center_x) / width_x;
    let dy = (y - center_y) / width_y;
    (-0.5 * (dx * dx + dy * dy)).exp()
}

fn field_value(
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
    is_b: bool,
    drift: f64,
) -> f64 {
    let x = column as f64 / (columns - 1) as f64;
    let y = row as f64 / (rows - 1) as f64;
    let shift = if is_b { 0.055 } else { 0.0 };
    let main = 3.2 * gaussian(x, y, 0.38 + drift + shift, 0.54, 0.13, 0.19);
    let secondary = (if is_b { 1.05 } else { 0.75 })
        * gaussian(x, y, 0.72 + drift + shift * 0.4, 0.29, 0.08, 0.11);
    0.12 + main + secondary
}

fn plane_value(row: usize, column: usize, is_b: bool) -> f64 {
    let value = field_value(row, column, 96, 128, is_b, 0.0);
    match (is_b, row, column) {
        (_, 44..=49, 6..=17) => 0.5,
        (false, 12..=21, 12..=23) => f64::NAN,
        (false, 30..=39, 12..=23) => FILL_VALUE,
        (true, 58..=67, 90..=101) => f64::INFINITY,
        (true, 74..=83, 90..=101) => f64::NEG_INFINITY,
        (true, 12..=21, 90..=101) => 0.0,
        _ => value,
    }
}

fn write(path: &Path, is_b: bool) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let field = file.create_group("field")?;
    const ROWS: usize = 96;
    const COLUMNS: usize = 128;
    let plane = (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| plane_value(row, column, is_b)))
        .collect::<Vec<_>>();
    let plane_dataset = array(&field, "plane", &[ROWS, COLUMNS], &plane)?;
    plane_dataset
        .new_attr::<f64>()
        .create("_FillValue")?
        .write_scalar(&FILL_VALUE)?;
    array(
        &field,
        "reference",
        &[ROWS, COLUMNS],
        &(0..ROWS)
            .flat_map(|row| {
                (0..COLUMNS).map(move |column| {
                    field_value(row, column, ROWS, COLUMNS, is_b, 0.0) * 0.85 + 0.15
                })
            })
            .collect::<Vec<_>>(),
    )?;
    array(
        &field,
        "zero_reference",
        &[ROWS, COLUMNS],
        &(0..ROWS)
            .flat_map(|row| {
                (0..COLUMNS).map(move |column| {
                    if is_b && column == 0 {
                        0.0
                    } else {
                        field_value(row, column, ROWS, COLUMNS, is_b, 0.0)
                    }
                })
            })
            .collect::<Vec<_>>(),
    )?;
    const CUBE_ROWS: usize = 48;
    const CUBE_COLUMNS: usize = 64;
    let cube = (0..3)
        .flat_map(|layer| {
            (0..CUBE_ROWS).flat_map(move |row| {
                (0..CUBE_COLUMNS).map(move |column| {
                    field_value(
                        row,
                        column,
                        CUBE_ROWS,
                        CUBE_COLUMNS,
                        is_b,
                        layer as f64 * 0.07,
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    array(&field, "cube", &[3, CUBE_ROWS, CUBE_COLUMNS], &cube)?;
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
