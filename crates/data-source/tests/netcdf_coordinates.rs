use h5jive_data::File;

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
fn put_name(bytes: &mut Vec<u8>, name: &str) {
    put_u32(bytes, name.len() as u32);
    bytes.extend_from_slice(name.as_bytes());
    bytes.resize(bytes.len().next_multiple_of(4), 0);
}

#[test]
fn classic_netcdf_coordinate_values_follow_dimension_indexes() {
    let mut bytes = b"CDF\x01".to_vec();
    put_u32(&mut bytes, 0); // numrecs
    put_u32(&mut bytes, 10); // dimension list
    put_u32(&mut bytes, 1);
    put_name(&mut bytes, "time");
    put_u32(&mut bytes, 3);
    put_u32(&mut bytes, 0); // no global attributes
    put_u32(&mut bytes, 0);
    put_u32(&mut bytes, 11); // variable list
    put_u32(&mut bytes, 2);
    let mut offsets = Vec::new();
    for name in ["time", "temperature"] {
        put_name(&mut bytes, name);
        put_u32(&mut bytes, 1); // one dimension
        put_u32(&mut bytes, 0); // time dimension
        put_u32(&mut bytes, 0); // no attributes
        put_u32(&mut bytes, 0);
        put_u32(&mut bytes, 5); // NC_FLOAT
        put_u32(&mut bytes, 12); // three f32 values
        offsets.push(bytes.len());
        put_u32(&mut bytes, 0); // data offset, filled below
    }
    for (variable, values) in [[0.0_f32, 1.5, 3.0], [10.0, 11.0, 12.0]]
        .into_iter()
        .enumerate()
    {
        let start = bytes.len() as u32;
        bytes[offsets[variable]..offsets[variable] + 4].copy_from_slice(&start.to_be_bytes());
        for value in values {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    let path = std::env::temp_dir().join(format!("h5jive-coordinate-{}.nc", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let file = File::open(&path).unwrap();
    assert_eq!(file.axis_names("/temperature").unwrap(), ["time"]);
    let axes = file.coordinate_axes("/temperature").unwrap();
    assert_eq!(axes[0].variable.as_deref(), Some("/time"));
    assert_eq!(axes[0].first, Some(0.0));
    assert_eq!(axes[0].last, Some(3.0));
    assert_eq!(file.region("/time", &[1], &[1]).unwrap().data, [1.5]);
    std::fs::remove_file(path).unwrap();
}
