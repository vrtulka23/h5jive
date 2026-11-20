use hdf5_metno::File;
use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = Path::new("examples/plugins/dataset_manipulation");
    fs::create_dir_all(directory)?;
    for (name, offset) in [
        ("manipulation_a.hdf5", 0_i16),
        ("manipulation_b.hdf5", 2_i16),
    ] {
        let path = directory.join(name);
        let file = File::create(&path)?;
        let group = file.create_group("measurements")?;
        let counts = (0..12).map(|value| value + offset).collect::<Vec<_>>();
        group
            .new_dataset::<i16>()
            .shape([3, 4])
            .create("counts")?
            .write_raw(&counts)?;
        let signal = (0..12)
            .map(|value| value as f32 * 0.5 + offset as f32)
            .collect::<Vec<_>>();
        group
            .new_dataset::<f32>()
            .shape([3, 4])
            .create("signal")?
            .write_raw(&signal)?;
        file.flush()?;
        println!("Created {}", path.display());
    }
    Ok(())
}
