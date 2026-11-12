//! Extract a compact, real GOME-2 Band 3 sample from a local Level 1B product.
use h5jive_data::File as SourceFile;
use hdf5_metno::{types::VarLenUnicode, Dataset, File, Group};
use std::{error::Error, path::PathBuf, str::FromStr};

const BAND: &str = "/MODE_NADIR/BAND_3";
const CHANNEL_START: usize = 480;
const CHANNEL_COUNT: usize = 64;

fn text_attr(dataset: &Dataset, name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    dataset
        .new_attr::<VarLenUnicode>()
        .create(name)?
        .write_scalar(&VarLenUnicode::from_str(value)?)?;
    Ok(())
}

fn root_attr(file: &File, name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    file.new_attr::<VarLenUnicode>()
        .create(name)?
        .write_scalar(&VarLenUnicode::from_str(value)?)?;
    Ok(())
}

fn copy_spatial(
    source: &SourceFile,
    output: &Group,
    name: &str,
    shape: &[usize],
    units: &str,
    fill: f32,
) -> Result<Dataset, Box<dyn Error>> {
    let input_path = format!("{BAND}/GEODATA/{name}");
    if source.dataset_info(&input_path)?.shape != shape {
        return Err(format!("unexpected shape for {input_path}").into());
    }
    let data: Vec<f32> = source
        .region(&input_path, &vec![0; shape.len()], shape)?
        .data
        .into_iter()
        .map(|value| value as f32)
        .collect();
    let dataset = output.new_dataset::<f32>().shape(shape).create(name)?;
    dataset.write_raw(&data)?;
    dataset
        .new_attr::<f32>()
        .create("_FillValue")?
        .write_scalar(&fill)?;
    text_attr(&dataset, "units", units)?;
    Ok(dataset)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let input =
        PathBuf::from(args.next().unwrap_or_else(|| {
            "../mock_datasets/GOME2/01/06/20130106101404Z_32260_gome.nc".into()
        }));
    let output = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "examples/gome2_band3_20130106_gome.nc".into()),
    );
    if args.next().is_some() {
        return Err("expected at most a source path and output path".into());
    }
    let source = SourceFile::open(&input)?;
    let radiance_path = format!("{BAND}/OBSERVATIONS/radiance");
    let source_shape = source.dataset_info(&radiance_path)?.shape;
    if source_shape.len() != 4
        || source_shape[0] != 1
        || source_shape[3] < CHANNEL_START + CHANNEL_COUNT
    {
        return Err(format!("unexpected radiance shape: {source_shape:?}").into());
    }
    let (lines, pixels) = (source_shape[1], source_shape[2]);
    let geo_shape = [1, lines, pixels];
    let bounds_shape = [1, lines, pixels, 4];
    let fill = source
        .region(&radiance_path, &[0, 0, 0, CHANNEL_START], &[1, 1, 1, 1])?
        .fill_value
        .ok_or("source radiance has no fill value")? as f32;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(&output)?;
    for (name, value) in [
        ("platform", "MetOp M02"),
        ("sensor", "GOME-2"),
        ("product_type", "GO2_L1B___"),
        ("orbit", "32260"),
        ("source_product", "20130106101404Z_32260_gome.nc"),
    ] {
        root_attr(&file, name, value)?;
    }
    let band = file.create_group("MODE_NADIR")?.create_group("BAND_3")?;
    let geodata = band.create_group("GEODATA")?;
    let observations = band.create_group("OBSERVATIONS")?;
    for (name, units) in [("latitude", "degrees_north"), ("longitude", "degrees_east")] {
        let dataset = copy_spatial(&source, &geodata, name, &geo_shape, units, fill)?;
        text_attr(&dataset, "standard_name", name)?;
        text_attr(&dataset, "bounds", &format!("{name}_bounds"))?;
        copy_spatial(
            &source,
            &geodata,
            &format!("{name}_bounds"),
            &bounds_shape,
            units,
            fill,
        )?;
    }
    let scan_path = format!("{BAND}/GEODATA/scan_direction");
    let scan: Vec<i32> = source
        .region(&scan_path, &[0, 0, 0], &geo_shape)?
        .data
        .into_iter()
        .map(|value| value as i32)
        .collect();
    let scan_output = geodata
        .new_dataset::<i32>()
        .shape(geo_shape)
        .create("scan_direction")?;
    scan_output.write_raw(&scan)?;
    text_attr(&scan_output, "description", "1 forward, 2 backward")?;

    let mut radiance = Vec::<f32>::with_capacity(lines * pixels * CHANNEL_COUNT);
    for first in (0..lines).step_by(16) {
        let count = (lines - first).min(16);
        radiance.extend(
            source
                .region(
                    &radiance_path,
                    &[0, first, 0, CHANNEL_START],
                    &[1, count, pixels, CHANNEL_COUNT],
                )?
                .data
                .into_iter()
                .map(|value| value as f32),
        );
    }
    let output_radiance = observations
        .new_dataset::<f32>()
        .shape([1, lines, pixels, CHANNEL_COUNT])
        .create("radiance")?;
    output_radiance.write_raw(&radiance)?;
    output_radiance
        .new_attr::<f32>()
        .create("_FillValue")?
        .write_scalar(&fill)?;
    output_radiance
        .new_attr::<u32>()
        .create("source_channel_start")?
        .write_scalar(&(CHANNEL_START as u32))?;
    text_attr(&output_radiance, "units", "photons/s.sr.cm2.nm")?;
    text_attr(
        &output_radiance,
        "coordinates",
        "../GEODATA/latitude ../GEODATA/longitude",
    )?;
    text_attr(
        &output_radiance,
        "dimensions",
        "time scan_line ground_pixel spectral_channel",
    )?;
    file.flush()?;
    println!(
        "Created {}: {lines} scan lines × {pixels} pixels × {CHANNEL_COUNT} channels",
        output.display()
    );
    Ok(())
}
