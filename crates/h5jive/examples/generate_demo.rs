//! Reproducible 1D–4D HDF5 swath fixture for the H5Jive viewer.
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

    file.flush()?;
    drop(file);
    let saved = File::open(&output)?;
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
