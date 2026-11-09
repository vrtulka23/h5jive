//! Minimal NetCDF classic writer for the reproducible GOME-2 orbit fixture.
use std::{error::Error, path::Path};

#[derive(Clone, Copy)]
enum Attribute<'a> {
    Text(&'a str),
    Float(f32),
}

struct Variable<'a> {
    name: &'a str,
    dimensions: &'a [u32],
    attributes: &'a [(&'a str, Attribute<'a>)],
    data: &'a [f32],
}

fn put_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Box<dyn Error>> {
    bytes.extend_from_slice(&u32::try_from(value)?.to_be_bytes());
    Ok(())
}

fn put_name(bytes: &mut Vec<u8>, name: &str) -> Result<(), Box<dyn Error>> {
    put_u32(bytes, name.len())?;
    bytes.extend_from_slice(name.as_bytes());
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    Ok(())
}

fn put_attributes(
    bytes: &mut Vec<u8>,
    attributes: &[(&str, Attribute<'_>)],
) -> Result<(), Box<dyn Error>> {
    if attributes.is_empty() {
        put_u32(bytes, 0)?;
        put_u32(bytes, 0)?;
        return Ok(());
    }
    put_u32(bytes, 12)?; // NC_ATTRIBUTE
    put_u32(bytes, attributes.len())?;
    for (name, value) in attributes {
        put_name(bytes, name)?;
        match value {
            Attribute::Text(text) => {
                put_u32(bytes, 2)?; // NC_CHAR
                put_u32(bytes, text.len())?;
                bytes.extend_from_slice(text.as_bytes());
                bytes.resize(bytes.len().next_multiple_of(4), 0);
            }
            Attribute::Float(number) => {
                put_u32(bytes, 5)?; // NC_FLOAT
                put_u32(bytes, 1)?;
                bytes.extend_from_slice(&number.to_be_bytes());
            }
        }
    }
    Ok(())
}

pub fn write_satellite_netcdf(
    path: &Path,
    shape: [usize; 3],
    latitude: &[f32],
    longitude: &[f32],
    latitude_bounds: &[f32],
    longitude_bounds: &[f32],
    radiance: &[f32],
    fill: f32,
) -> Result<(), Box<dyn Error>> {
    let expected = shape.iter().product::<usize>();
    if latitude.len() != expected
        || longitude.len() != expected
        || radiance.len() != expected
        || latitude_bounds.len() != expected * 4
        || longitude_bounds.len() != expected * 4
    {
        return Err("satellite NetCDF arrays have inconsistent shapes".into());
    }
    let variables = [
        Variable {
            name: "latitude",
            dimensions: &[0, 1, 2],
            attributes: &[
                ("standard_name", Attribute::Text("latitude")),
                ("units", Attribute::Text("degrees_north")),
                ("bounds", Attribute::Text("latitude_bounds")),
            ],
            data: latitude,
        },
        Variable {
            name: "longitude",
            dimensions: &[0, 1, 2],
            attributes: &[
                ("standard_name", Attribute::Text("longitude")),
                ("units", Attribute::Text("degrees_east")),
                ("bounds", Attribute::Text("longitude_bounds")),
            ],
            data: longitude,
        },
        Variable {
            name: "so2_slant_column",
            dimensions: &[0, 1, 2],
            attributes: &[
                ("coordinates", Attribute::Text("latitude longitude")),
                ("units", Attribute::Text("DU")),
                ("_FillValue", Attribute::Float(fill)),
            ],
            data: radiance,
        },
        Variable {
            name: "latitude_bounds",
            dimensions: &[0, 1, 2, 3],
            attributes: &[
                ("standard_name", Attribute::Text("latitude")),
                ("units", Attribute::Text("degrees_north")),
            ],
            data: latitude_bounds,
        },
        Variable {
            name: "longitude_bounds",
            dimensions: &[0, 1, 2, 3],
            attributes: &[
                ("standard_name", Attribute::Text("longitude")),
                ("units", Attribute::Text("degrees_east")),
            ],
            data: longitude_bounds,
        },
    ];
    let mut bytes = b"CDF\x01".to_vec();
    put_u32(&mut bytes, 0)?; // fixed-size dimensions, no record variables
    put_u32(&mut bytes, 10)?; // NC_DIMENSION
    put_u32(&mut bytes, 4)?;
    for (name, length) in [
        ("time", shape[0]),
        ("scanline", shape[1]),
        ("ground_pixel", shape[2]),
        ("corner", 4),
    ] {
        put_name(&mut bytes, name)?;
        put_u32(&mut bytes, length)?;
    }
    put_attributes(&mut bytes, &[("Conventions", Attribute::Text("CF-1.10"))])?;
    put_u32(&mut bytes, 11)?; // NC_VARIABLE
    put_u32(&mut bytes, variables.len())?;
    let mut offsets = Vec::with_capacity(variables.len());
    for variable in &variables {
        put_name(&mut bytes, variable.name)?;
        put_u32(&mut bytes, variable.dimensions.len())?;
        for &dimension in variable.dimensions {
            put_u32(&mut bytes, dimension as usize)?;
        }
        put_attributes(&mut bytes, variable.attributes)?;
        put_u32(&mut bytes, 5)?; // NC_FLOAT
        put_u32(&mut bytes, variable.data.len() * 4)?;
        offsets.push(bytes.len());
        put_u32(&mut bytes, 0)?; // data offset, patched below
    }
    for (variable, offset) in variables.iter().zip(offsets) {
        let position = u32::try_from(bytes.len())?;
        bytes[offset..offset + 4].copy_from_slice(&position.to_be_bytes());
        for value in variable.data {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    std::fs::write(path, bytes)?;
    Ok(())
}
