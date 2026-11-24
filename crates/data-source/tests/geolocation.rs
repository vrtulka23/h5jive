use h5jive_data::{File, GeolocationRule, ScanDirectionRule, ScanFilter};

const RADIANCE: &str = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";

#[test]
fn bundled_band3_has_real_geolocation_bounds_and_scan_flags() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
    let file = File::open(&path).unwrap();
    assert_eq!(file.dataset_info(RADIANCE).unwrap().shape, [1, 625, 32, 64]);
    let geo = file.geolocation(RADIANCE).unwrap().unwrap();
    assert_eq!(geo.latitude, "/MODE_NADIR/BAND_3/GEODATA/latitude");
    assert_eq!(geo.longitude, "/MODE_NADIR/BAND_3/GEODATA/longitude");
    assert!(geo.latitude_bounds.is_some());
    assert!(geo.longitude_bounds.is_some());
    let rule = GeolocationRule {
        file_glob: "*_gome.nc".into(),
        platform: "MetOp M0?".into(),
        sensor: "GOME-2".into(),
        product_type: "GO2_L1B___".into(),
        observation_group: "OBSERVATIONS".into(),
        coordinate_group: "GEODATA".into(),
        latitude: "latitude".into(),
        longitude: "longitude".into(),
        latitude_bounds: "latitude_bounds".into(),
        longitude_bounds: "longitude_bounds".into(),
        time_coordinate: String::new(),
        scan_direction: Some(ScanDirectionRule {
            group: "coordinate".into(),
            variable: "scan_direction".into(),
            forward_values: vec![1],
            backward_values: vec![2],
            forward_groups: vec![],
            backward_groups: vec![],
            default_filter: ScanFilter::Forward,
        }),
    };
    let geo = file
        .geolocation_with_rules(RADIANCE, &[rule])
        .unwrap()
        .unwrap();
    let scan = geo.scan_direction.unwrap();
    assert_eq!(scan.default_filter, ScanFilter::Forward);
    let flags = file
        .region(&scan.path, &[0, 0, 0], &[1, 625, 32])
        .unwrap()
        .data;
    assert!(flags.contains(&1.0));
    assert!(flags.contains(&2.0));
    let lat = file
        .region(&geo.latitude, &[0, 100, 0], &[1, 1, 2])
        .unwrap()
        .data;
    let lon = file
        .region(&geo.longitude, &[0, 100, 0], &[1, 1, 2])
        .unwrap()
        .data;
    assert!(lat.iter().all(|value| (-90.0..=90.0).contains(value)));
    assert!(lon.iter().all(|value| (-180.0..=180.0).contains(value)));
    assert_ne!(lon[0], lon[1]);
    assert!(file.geolocation(&geo.latitude).unwrap().is_none());
    assert!(file.geolocation(&geo.longitude).unwrap().is_none());
}

#[test]
fn level2_group_layouts_resolve_pixel_centers() {
    for (filename, observation_group, coordinate_group, variable) in [
        ("PACE_OCI.20240426T074015.L2.OC_AOP.V3_0.nc", "geophysical_data", "navigation_data", "chlor_a"),
        ("AERDT_L2_VIIRS_SNPP.A2025181.1812.021.2025191191124.nc", "geophysical_data", "geolocation_data", "Optical_Depth_Land_And_Ocean"),
        ("S5P_OFFL_L2__NO2____20201007T202447_20201007T220617_15471_01_020200_20210515T213556.nc", "PRODUCT", "PRODUCT", "nitrogendioxide_tropospheric_column"),
        ("S5P_OFFL_L2__O3_____20201007T202447_20201007T220617_15471_01_020200_20210515T213556.nc", "PRODUCT", "PRODUCT", "ozone_total_vertical_column"),
    ] {
        let directory = std::env::temp_dir().join(format!(
            "h5jive-level2-geolocation-{}-{}",
            std::process::id(),
            coordinate_group
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(filename);
        let mut output = netcdf::create(&path).unwrap();
        output.add_dimension("row", 2).unwrap();
        output.add_dimension("column", 2).unwrap();
        output.add_group(observation_group).unwrap();
        if coordinate_group != observation_group {
            output.add_group(coordinate_group).unwrap();
        }
        output
            .add_variable::<f32>(&format!("{observation_group}/{variable}"), &["row", "column"])
            .unwrap()
            .put_values(&[1.0, 2.0, 3.0, 4.0], ..)
            .unwrap();
        for name in ["latitude", "longitude"] {
            output
                .add_variable::<f64>(&format!("{coordinate_group}/{name}"), &["row", "column"])
                .unwrap()
                .put_values(&[0.0, 1.0, 2.0, 3.0], ..)
                .unwrap();
        }
        output.close().unwrap();

        let file = File::open(&path).unwrap();
        let rule = GeolocationRule {
            file_glob: match coordinate_group {
                "PRODUCT" => "S5P_*_L2__*.nc",
                "geolocation_data" => "AERDT_L2_VIIRS_*.nc",
                _ => "*.L2*.nc",
            }
            .into(),
            platform: String::new(),
            sensor: String::new(),
            product_type: String::new(),
            observation_group: observation_group.into(),
            coordinate_group: coordinate_group.into(),
            latitude: "latitude".into(),
            longitude: "longitude".into(),
            latitude_bounds: String::new(),
            longitude_bounds: String::new(),
            time_coordinate: String::new(),
            scan_direction: None,
        };
        let observation = format!("/{observation_group}/{variable}");
        let geolocation = file
            .geolocation_with_rules(&observation, &[rule])
            .unwrap()
            .unwrap();
        assert_eq!(geolocation.latitude, format!("/{coordinate_group}/latitude"));
        assert_eq!(geolocation.longitude, format!("/{coordinate_group}/longitude"));
        drop(file);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

#[test]
fn ancestor_groups_classify_scan_direction_without_pixel_flags() {
    let directory = std::env::temp_dir().join(format!("h5jive-group-scans-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("ER2_RPRO_GOM_L1B_test.nc");
    let mut output = netcdf::create(&path).unwrap();
    output.add_dimension("row", 2).unwrap();
    output.add_dimension("column", 2).unwrap();
    for mode in ["MODE_NADIR", "MODE_NARROW_SWATH_BACKSCAN"] {
        output.add_group(mode).unwrap();
        output.add_group(&format!("{mode}/BAND_3")).unwrap();
        for group in ["OBSERVATIONS", "GEODATA"] {
            output.add_group(&format!("{mode}/BAND_3/{group}")).unwrap();
        }
        for (group, name) in [
            ("OBSERVATIONS", "radiance"),
            ("GEODATA", "latitude"),
            ("GEODATA", "longitude"),
        ] {
            output
                .add_variable::<f64>(&format!("{mode}/BAND_3/{group}/{name}"), &["row", "column"])
                .unwrap()
                .put_values(&[1.0, 2.0, 3.0, 4.0], ..)
                .unwrap();
        }
    }
    output.close().unwrap();
    let rule = GeolocationRule {
        file_glob: "ER2_RPRO_GOM_L1B*.nc".into(),
        platform: String::new(),
        sensor: String::new(),
        product_type: String::new(),
        observation_group: "OBSERVATIONS".into(),
        coordinate_group: "GEODATA".into(),
        latitude: "latitude".into(),
        longitude: "longitude".into(),
        latitude_bounds: String::new(),
        longitude_bounds: String::new(),
        time_coordinate: String::new(),
        scan_direction: Some(ScanDirectionRule {
            group: "coordinate".into(),
            variable: String::new(),
            forward_values: vec![],
            backward_values: vec![],
            forward_groups: vec!["MODE_NADIR".into()],
            backward_groups: vec!["MODE_NARROW_SWATH_BACKSCAN".into()],
            default_filter: ScanFilter::Both,
        }),
    };
    let file = File::open(&path).unwrap();
    for (mode, expected) in [
        ("MODE_NADIR", h5jive_data::ScanDirectionClass::Forward),
        (
            "MODE_NARROW_SWATH_BACKSCAN",
            h5jive_data::ScanDirectionClass::Backward,
        ),
    ] {
        let observation = format!("/{mode}/BAND_3/OBSERVATIONS/radiance");
        let geo = file
            .geolocation_with_rules(&observation, std::slice::from_ref(&rule))
            .unwrap()
            .unwrap();
        let scan = geo.scan_direction.unwrap();
        assert_eq!(scan.constant, Some(expected));
        assert_eq!(scan.classify(f64::NAN, None), expected);
    }
    drop(file);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
