//! Reproducible 1D–4D HDF5 swath fixture for the H5Jive viewer.
#[path = "generate_demo/netcdf_classic.rs"]
mod netcdf_classic;
use hdf5_metno::{types::VarLenUnicode, Dataset, File, Group};
use std::{error::Error, path::PathBuf, str::FromStr};

const NT: usize = 8;
const NY: usize = 32;
const NX: usize = 48;
const NW: usize = 16;
const FILL: f64 = -9999.0;

#[derive(Clone, Copy)]
struct Region {
    center: &'static [f64],
    radius: &'static [f64],
    value: f64,
}
const DISKS: [Region; 4] = [
    Region {
        center: &[8., 11.],
        radius: &[3., 4.],
        value: f64::NAN,
    },
    Region {
        center: &[18., 34.],
        radius: &[3., 4.],
        value: f64::INFINITY,
    },
    Region {
        center: &[24., 37.],
        radius: &[3., 4.],
        value: f64::NEG_INFINITY,
    },
    Region {
        center: &[25., 12.],
        radius: &[3., 4.],
        value: FILL,
    },
];
const SPHERES: [Region; 4] = [
    Region {
        center: &[8., 11., 4.],
        radius: &[3., 4., 2.5],
        value: f64::NAN,
    },
    Region {
        center: &[18., 34., 10.],
        radius: &[3., 4., 2.5],
        value: f64::INFINITY,
    },
    Region {
        center: &[24., 37., 5.],
        radius: &[3., 4., 2.5],
        value: f64::NEG_INFINITY,
    },
    Region {
        center: &[25., 12., 11.],
        radius: &[3., 4., 2.5],
        value: FILL,
    },
];
const HYPERSPHERES: [Region; 4] = [
    Region {
        center: &[2., 8., 11., 4.],
        radius: &[1.5, 4., 5., 3.],
        value: f64::NAN,
    },
    Region {
        center: &[5., 18., 34., 10.],
        radius: &[1.5, 4., 5., 3.],
        value: f64::INFINITY,
    },
    Region {
        center: &[5., 24., 37., 5.],
        radius: &[1.5, 4., 5., 3.],
        value: f64::NEG_INFINITY,
    },
    Region {
        center: &[6., 25., 12., 11.],
        radius: &[1.5, 4., 5., 3.],
        value: FILL,
    },
];

fn sphere_value(position: &[f64], regions: &[Region]) -> Option<f64> {
    regions
        .iter()
        .find(|region| {
            position
                .iter()
                .zip(region.center)
                .zip(region.radius)
                .map(|((&p, &c), &r)| ((p - c) / r).powi(2))
                .sum::<f64>()
                <= 1.0
        })
        .map(|region| region.value)
}
fn fraction(index: usize, length: usize) -> f64 {
    index as f64 / (length - 1) as f64
}
fn smooth(axes: &[f64]) -> f64 {
    let sum: f64 = axes.iter().sum();
    -0.7 + 1.4 * sum / axes.len() as f64 + 0.04 * (sum * std::f64::consts::TAU).sin()
}

fn read_gome2_orbit() -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>), Box<dyn Error>>
{
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/gome2_20090730_095156.dat"),
    )?;
    let (mut values, mut latitude, mut longitude, mut latitude_bounds, mut longitude_bounds) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for line in source
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
    {
        let fields = line
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()?;
        if fields.len() != 47 {
            return Err(format!(
                "unexpected GOME-2 orbit record with {} fields",
                fields.len()
            )
            .into());
        }
        // SCDbgc is the measured GOME-2 SO2 slant column after background correction.
        values.push(if fields[17] == -99.0 {
            FILL
        } else {
            fields[17]
        });
        latitude.push(fields[7]);
        longitude.push(fields[12]);
        // GOME-2 writes corners as two pairs (A, B, C, D); draw APIs expect a
        // perimeter walk, so swap the second pair to A, B, D, C.
        for corner in [0, 1, 3, 2] {
            latitude_bounds.push(fields[3 + corner]);
            longitude_bounds.push(fields[8 + corner]);
        }
    }
    if values.is_empty() {
        return Err("GOME-2 orbit has no pixel records".into());
    }
    Ok((
        values,
        latitude,
        longitude,
        latitude_bounds,
        longitude_bounds,
    ))
}

fn write_gome2_sample(out: &Group) -> Result<usize, Box<dyn Error>> {
    let (values, latitude, longitude, latitude_bounds, longitude_bounds) = read_gome2_orbit()?;
    let count = values.len();
    let g = out.create_group("gome2")?;
    // The supplied SO2 orbit extract contains one geolocated forward pixel per
    // six-second scan record. Preserve the scanline axis and the singleton
    // ground-pixel axis instead of flattening both into `observation`.
    let shape = [1, count, 1];
    write_array(
        &g,
        "so2_slant_column",
        &shape,
        &values,
        "DU",
        "latitude longitude",
        true,
    )?;
    write_array(
        &g,
        "latitude",
        &shape,
        &latitude,
        "degrees_north",
        "",
        false,
    )?;
    write_array(
        &g,
        "longitude",
        &shape,
        &longitude,
        "degrees_east",
        "",
        false,
    )?;
    write_array(
        &g,
        "latitude_bounds",
        &[1, count, 1, 4],
        &latitude_bounds,
        "degrees_north",
        "",
        false,
    )?;
    write_array(
        &g,
        "longitude_bounds",
        &[1, count, 1, 4],
        &longitude_bounds,
        "degrees_east",
        "",
        false,
    )?;
    text_attr(&g.dataset("latitude")?, "standard_name", "latitude")?;
    text_attr(&g.dataset("longitude")?, "standard_name", "longitude")?;
    text_attr(&g.dataset("latitude")?, "bounds", "latitude_bounds")?;
    text_attr(&g.dataset("longitude")?, "bounds", "longitude_bounds")?;
    Ok(count)
}
fn text_attr(dataset: &Dataset, name: &str, value: &str) -> Result<(), Box<dyn Error>> {
    let value = VarLenUnicode::from_str(value)?;
    dataset
        .new_attr::<VarLenUnicode>()
        .create(name)?
        .write_scalar(&value)?;
    Ok(())
}
fn write_array(
    group: &Group,
    name: &str,
    shape: &[usize],
    data: &[f64],
    units: &str,
    axes: &str,
    with_fill: bool,
) -> Result<(), Box<dyn Error>> {
    let dataset = group.new_dataset::<f64>().shape(shape).create(name)?;
    dataset.write_raw(data)?;
    text_attr(&dataset, "units", units)?;
    text_attr(&dataset, "coordinates", axes)?;
    if with_fill {
        dataset
            .new_attr::<f64>()
            .create("_FillValue")?
            .write_scalar(&FILL)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let output = PathBuf::from(
        arguments
            .next()
            .unwrap_or_else(|| "examples/space_demo.hdf5".into()),
    );
    if arguments.next().is_some() {
        return Err("expected at most one output path".into());
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(&output)?;
    let coordinates = file.create_group("coordinates")?;
    let observations = file.create_group("observations")?;

    let time: Vec<f64> = (0..NT).map(|n| n as f64 * 10.0).collect();
    let scanline: Vec<f64> = (0..NY)
        .map(|n| -12.0 + n as f64 * 24.0 / (NY - 1) as f64)
        .collect();
    let ground_pixel: Vec<f64> = (0..NX)
        .map(|n| -24.0 + n as f64 * 48.0 / (NX - 1) as f64)
        .collect();
    let wavelength: Vec<f64> = (0..NW).map(|n| 400.0 + n as f64 * 20.0).collect();
    for (name, data, unit) in [
        ("time", &time, "s"),
        ("scanline", &scanline, "km"),
        ("ground_pixel", &ground_pixel, "km"),
        ("wavelength", &wavelength, "nm"),
    ] {
        write_array(&coordinates, name, &[data.len()], data, unit, name, false)?;
    }

    let curve: Vec<f64> = (0..NT).map(|t| smooth(&[fraction(t, NT)])).collect();
    write_array(
        &observations,
        "time_curve_1d",
        &[NT],
        &curve,
        "1",
        "time",
        true,
    )?;

    let swath: Vec<f64> = (0..NY)
        .flat_map(|y| {
            (0..NX).map(move |x| {
                sphere_value(&[y as f64, x as f64], &DISKS)
                    .unwrap_or_else(|| smooth(&[fraction(y, NY), fraction(x, NX)]))
            })
        })
        .collect();
    write_array(
        &observations,
        "swath_2d",
        &[NY, NX],
        &swath,
        "1",
        "scanline ground_pixel",
        true,
    )?;

    let cube: Vec<f64> = (0..NY)
        .flat_map(|y| {
            (0..NX).flat_map(move |x| {
                (0..NW).map(move |w| {
                    sphere_value(&[y as f64, x as f64, w as f64], &SPHERES).unwrap_or_else(|| {
                        smooth(&[fraction(y, NY), fraction(x, NX), fraction(w, NW)])
                    })
                })
            })
        })
        .collect();
    write_array(
        &observations,
        "spectral_cube_3d",
        &[NY, NX, NW],
        &cube,
        "1",
        "scanline ground_pixel wavelength",
        true,
    )?;

    let time_cube: Vec<f64> = (0..NT)
        .flat_map(|t| {
            (0..NY).flat_map(move |y| {
                (0..NX).flat_map(move |x| {
                    (0..NW).map(move |w| {
                        sphere_value(&[t as f64, y as f64, x as f64, w as f64], &HYPERSPHERES)
                            .unwrap_or_else(|| {
                                smooth(&[
                                    fraction(t, NT),
                                    fraction(y, NY),
                                    fraction(x, NX),
                                    fraction(w, NW),
                                ])
                            })
                    })
                })
            })
        })
        .collect();
    write_array(
        &observations,
        "time_spectral_cube_4d",
        &[NT, NY, NX, NW],
        &time_cube,
        "1",
        "time scanline ground_pixel wavelength",
        true,
    )?;

    // A moving, global satellite swath with curvilinear geolocation for every sample.
    let satellite = file.create_group("satellite")?;
    let mut latitude = Vec::with_capacity(NT * NY * NX);
    let mut longitude = Vec::with_capacity(NT * NY * NX);
    let mut latitude_bounds = Vec::with_capacity(NT * NY * NX * 4);
    let mut longitude_bounds = Vec::with_capacity(NT * NY * NX * 4);
    let mut radiance = Vec::with_capacity(NT * NY * NX);
    let geo = |t: usize, y: f64, x: f64| {
        let phase = y * 0.10 + t as f64 * 0.55;
        let base_lon = -160.0 + 45.0 * t as f64 + 0.8 * y;
        let base_lat = 42.0 * phase.sin();
        let tangent_lon: f64 = 0.8;
        let tangent_lat = 4.2 * phase.cos();
        let norm = (tangent_lon * tangent_lon + tangent_lat * tangent_lat).sqrt();
        let (along_lon, along_lat) = (tangent_lon / norm, tangent_lat / norm);
        let cross_lon = -along_lat;
        let cross_lat = along_lon;
        let cross = (x - (NX as f64 - 1.0) * 0.5) * 0.42;
        (base_lat + cross * cross_lat, base_lon + cross * cross_lon)
    };
    for t in 0..NT {
        for y in 0..NY {
            for x in 0..NX {
                let (lat, lon) = geo(t, y as f64, x as f64);
                let lon = (lon + 180.0).rem_euclid(360.0) - 180.0;
                let mut value = 0.8
                    + 0.35 * (2.0 * lon.to_radians()).sin() * lat.to_radians().cos()
                    + 0.2 * (3.0 * lat.to_radians() + t as f64 * 0.2).cos();
                if t == 1 && (7..=10).contains(&y) && (10..=15).contains(&x) {
                    value = f64::NAN;
                } else if t == 3 && (15..=18).contains(&y) && (28..=32).contains(&x) {
                    value = f64::INFINITY;
                } else if t == 5 && (20..=23).contains(&y) && (6..=11).contains(&x) {
                    value = f64::NEG_INFINITY;
                } else if t == 7 && (3..=6).contains(&y) && (35..=40).contains(&x) {
                    value = FILL;
                }
                latitude.push(lat);
                longitude.push(lon);
                // Adjacent samples share the same locally rectangular along-track/cross-track grid.
                let phase = y as f64 * 0.10 + t as f64 * 0.55;
                let dlat = 4.2 * phase.cos();
                let norm = (0.8_f64 * 0.8 + dlat * dlat).sqrt();
                let along = [0.8 / norm * 0.5, dlat / norm * 0.5];
                let cross = [-dlat / norm * 0.21, 0.8 / norm * 0.21];
                for (along_sign, cross_sign) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)]
                {
                    let corner_lat = lat + along_sign * along[1] + cross_sign * cross[1];
                    let corner_lon = lon + along_sign * along[0] + cross_sign * cross[0];
                    latitude_bounds.push(corner_lat.clamp(-82.0, 82.0));
                    longitude_bounds.push((corner_lon + 180.0).rem_euclid(360.0) - 180.0);
                }
                radiance.push(value);
            }
        }
    }
    let shape = [NT, NY, NX];
    for (name, values, units, standard_name) in [
        (
            "latitude_bounds",
            &latitude_bounds,
            "degrees_north",
            "latitude",
        ),
        (
            "longitude_bounds",
            &longitude_bounds,
            "degrees_east",
            "longitude",
        ),
    ] {
        write_array(
            &satellite,
            name,
            &[NT, NY, NX, 4],
            values,
            units,
            "time scanline ground_pixel corner",
            false,
        )?;
        text_attr(&satellite.dataset(name)?, "standard_name", standard_name)?;
        if standard_name == "latitude" {
            text_attr(&satellite.dataset(name)?, "bounds", "latitude_bounds")?;
        } else {
            text_attr(&satellite.dataset(name)?, "bounds", "longitude_bounds")?;
        }
    }
    for (name, values, units, standard_name) in [
        ("latitude", &latitude, "degrees_north", "latitude"),
        ("longitude", &longitude, "degrees_east", "longitude"),
    ] {
        write_array(
            &satellite,
            name,
            &shape,
            values,
            units,
            "time scanline ground_pixel",
            false,
        )?;
        text_attr(&satellite.dataset(name)?, "standard_name", standard_name)?;
    }
    write_array(
        &satellite,
        "synthetic_radiance",
        &shape,
        &radiance,
        "W m-2 sr-1 um-1",
        "latitude longitude",
        true,
    )?;
    text_attr(
        &satellite.dataset("synthetic_radiance")?,
        "dimensions",
        "time scanline ground_pixel",
    )?;
    let gome_pixels = write_gome2_sample(&satellite)?;

    file.flush()?;
    drop(file);
    let saved = File::open(&output)?;
    assert_eq!(
        saved.dataset("satellite/gome2/so2_slant_column")?.shape(),
        [1, gome_pixels, 1]
    );
    let read_gome = |name: &str| -> Result<Vec<f32>, Box<dyn Error>> {
        Ok(saved
            .dataset(&format!("satellite/gome2/{name}"))?
            .read_raw::<f64>()?
            .into_iter()
            .map(|x| x as f32)
            .collect())
    };
    let netcdf_output = output.with_file_name("space_observation.nc");
    netcdf_classic::write_satellite_netcdf(
        &netcdf_output,
        [1, gome_pixels, 1],
        &read_gome("latitude")?,
        &read_gome("longitude")?,
        &read_gome("latitude_bounds")?,
        &read_gome("longitude_bounds")?,
        &read_gome("so2_slant_column")?,
        FILL as f32,
    )?;
    let curve = saved
        .dataset("observations/time_curve_1d")?
        .read_raw::<f64>()?;
    assert!(curve.iter().any(|&value| value < 0.0));
    assert!(curve.iter().any(|&value| value > 0.0));
    for (name, shape) in [
        ("swath_2d", vec![NY, NX]),
        ("spectral_cube_3d", vec![NY, NX, NW]),
        ("time_spectral_cube_4d", vec![NT, NY, NX, NW]),
    ] {
        let dataset = saved.dataset(&format!("observations/{name}"))?;
        assert_eq!(dataset.shape(), shape);
        assert_eq!(dataset.attr("_FillValue")?.read_scalar::<f64>()?, FILL);
        let values = dataset.read_raw::<f64>()?;
        let nan = values.iter().filter(|x| x.is_nan()).count();
        let positive_inf = values.iter().filter(|&&x| x == f64::INFINITY).count();
        let negative_inf = values.iter().filter(|&&x| x == f64::NEG_INFINITY).count();
        let fill = values.iter().filter(|&&x| x == FILL).count();
        assert!(nan > 0 && positive_inf > 0 && negative_inf > 0 && fill > 0);
        let ordinary: Vec<f64> = values
            .iter()
            .copied()
            .filter(|x| x.is_finite() && *x != FILL)
            .collect();
        assert!(ordinary.iter().all(|&x| (-0.75..=0.75).contains(&x)));
        assert!(ordinary.iter().any(|&x| x < 0.0));
        assert!(ordinary.iter().any(|&x| x > 0.0));
        println!("  {name}: NaN {nan}, +Inf {positive_inf}, -Inf {negative_inf}, fill {fill}");
    }
    println!(
        "Created {} (1D {NT}; 2D {NY}×{NX}; 3D {NY}×{NX}×{NW}; 4D {NT}×{NY}×{NX}×{NW})",
        output.display()
    );
    Ok(())
}
