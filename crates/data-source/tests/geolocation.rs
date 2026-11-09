use h5jive_data::File;

#[test]
fn satellite_examples_expose_curvilinear_geolocation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for (name, radiance, latitude, longitude, shape, start, count) in [
        (
            "space_demo.hdf5",
            "/satellite/synthetic_radiance",
            "/satellite/latitude",
            "/satellite/longitude",
            [8, 32, 48],
            [2, 10, 8],
            [1, 1, 2],
        ),
        (
            "space_observation.nc",
            "/so2_slant_column",
            "/latitude",
            "/longitude",
            [1, 10461, 1],
            [0, 10, 0],
            [1, 2, 1],
        ),
    ] {
        let file = File::open(&root.join(name)).unwrap();
        let geo = file.geolocation(radiance).unwrap().unwrap();
        assert_eq!(geo.latitude, latitude);
        assert_eq!(geo.longitude, longitude);
        assert_eq!(file.dataset_info(radiance).unwrap().shape, shape);
        let lat = file.region(&geo.latitude, &start, &count).unwrap().data;
        let lon = file.region(&geo.longitude, &start, &count).unwrap().data;
        assert!(lat.iter().all(|value| (-90.0..=90.0).contains(value)));
        assert!(lon.iter().all(|value| (-180.0..=180.0).contains(value)));
        assert_ne!(lon[0], lon[1]);
        assert!(file.geolocation(latitude).unwrap().is_none());
        assert!(file.geolocation(longitude).unwrap().is_none());
    }
}
