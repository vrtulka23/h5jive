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
        scan_direction: Some(ScanDirectionRule {
            group: "coordinate".into(),
            variable: "scan_direction".into(),
            forward_values: vec![1],
            backward_values: vec![2],
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
