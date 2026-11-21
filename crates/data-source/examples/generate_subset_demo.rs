use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("examples/plugins/spatiotemporal_subset/products");
    std::fs::create_dir_all(&root)?;
    for (orbit, offset, wind) in [(101, 0.0, 4.0_f32), (102, 0.7, 6.0), (103, 1.4, 8.0)] {
        let path = root.join(format!("north_sea_2024010{}_orbit{orbit}.nc", orbit - 100));
        let mut file = netcdf::create(&path)?;
        file.add_dimension("scan", 5)?;
        file.add_dimension("pixel", 6)?;
        let latitude = (0..5)
            .flat_map(|row| (0..6).map(move |_| 53.0 + offset + row as f64 * 0.6))
            .collect::<Vec<_>>();
        let longitude = (0..5)
            .flat_map(|_| (0..6).map(|col| 1.0 + col as f64 * 0.8))
            .collect::<Vec<_>>();
        file.add_variable::<f64>("latitude", &["scan", "pixel"])?
            .put_values(&latitude, ..)?;
        file.add_variable::<f64>("longitude", &["scan", "pixel"])?
            .put_values(&longitude, ..)?;
        file.variable_mut("latitude")
            .unwrap()
            .put_attribute("units", "degrees_north")?;
        file.variable_mut("longitude")
            .unwrap()
            .put_attribute("units", "degrees_east")?;
        for (name, delta) in [("u10", 0.0_f32), ("v10", -2.0)] {
            let mut variable = file.add_variable::<f32>(name, &["scan", "pixel"])?;
            variable.put_attribute("units", "m s-1")?;
            variable.put_attribute("long_name", format!("{name} wind component"))?;
            let values = (0..30)
                .map(|i| wind + delta + (i % 6) as f32 * 0.2)
                .collect::<Vec<_>>();
            variable.put_values(&values, ..)?;
        }
        file.add_attribute("title", "Synthetic North Sea wind swath")?;
        file.close()?;
        println!("{}", path.display());
    }
    Ok(())
}
