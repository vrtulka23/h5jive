use super::super::super::*;

#[cfg(test)]
mod tests {
    use super::super::geospatial_point_budget;
    use super::*;
    #[test]
    fn sampling_includes_both_ends_without_duplicates() {
        assert_eq!(sampled_indexes(48, 5), [0, 11, 23, 35, 47]);
        assert_eq!(sampled_indexes(3, 20), [0, 1, 2]);
    }

    #[test]
    fn zoom_raises_detail_until_every_pixel_is_available() {
        assert_eq!(geospatial_point_budget(30_000, 4.0, 1.0, 1_681_200), 30_000);
        assert_eq!(
            geospatial_point_budget(30_000, 4.0, 2.0, 1_681_200),
            120_000
        );
        assert_eq!(
            geospatial_point_budget(30_000, 4.0, 4.0, 1_681_200),
            480_000
        );
        assert_eq!(
            geospatial_point_budget(30_000, 4.0, 8.0, 1_681_200),
            1_681_200
        );
    }

    #[test]
    fn bundled_band3_has_values_and_footprints() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let file = File::open(&path).unwrap();
        let dataset = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";
        let geo = file
            .geolocation_with_rules(dataset, &Settings::load().unwrap().dataset_rules)
            .unwrap()
            .unwrap();
        let shape = file.dataset_info(dataset).unwrap().shape;
        let data = load_map_data(&file, dataset, &shape, &geo, Some(0), 32, 30_000).unwrap();
        assert_eq!(data.points.len(), 15_574);
        assert!(data.points.iter().all(|point| point.footprint.is_some()));
        assert!(data
            .points
            .iter()
            .all(|point| (-90.0..=90.0).contains(&point.latitude)));
        assert!(data.range.is_some_and(|(min, max)| min < max));
        assert!(data
            .points
            .iter()
            .any(|point| point.scan_direction == Some(h5jive_data::ScanDirectionClass::Forward)));
        assert!(data
            .points
            .iter()
            .any(|point| point.scan_direction == Some(h5jive_data::ScanDirectionClass::Backward)));
        let controls = GeoViewControls::new(true);
        assert_eq!(controls.applied.scan_filter, ScanFilter::Both);
        assert!(data
            .points
            .iter()
            .all(|point| controls.applied.scan_filter.allows(point.scan_direction)));
    }

    #[test]
    fn gome2_binning_uses_its_native_corner_orientation() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let file = File::open(&path).unwrap();
        let dataset = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";
        let geo = file
            .geolocation_with_rules(dataset, &Settings::load().unwrap().dataset_rules)
            .unwrap()
            .unwrap();
        assert!(geo.scan_direction.is_some());
        let shape = file.dataset_info(dataset).unwrap().shape;
        let map = load_map_data(&file, dataset, &shape, &geo, Some(0), 32, 500).unwrap();
        assert!(map.aggregated);
        assert!(map
            .points
            .iter()
            .all(|point| point.scan_direction.is_some()));
        assert!(
            map.points
                .iter()
                .filter(|point| point.footprint.is_some())
                .count()
                > map.points.len() / 2
        );
        assert!(map.range.is_some_and(|(min, max)| min < max));
    }

    #[test]
    fn s5p_radiance_bands_map_with_pixel_footprints_when_available() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mock_datasets/S5P");
        if !directory.is_dir() {
            return;
        }
        for band in [3, 4, 5] {
            let prefix = format!("S5P_OFFL_L1B_RA_BD{band}_");
            let fixture = std::fs::read_dir(&directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
                });
            let Some(fixture) = fixture else {
                continue;
            };
            let file = File::open(&fixture).unwrap();
            let dataset = format!("/BAND{band}_RADIANCE/STANDARD_MODE/OBSERVATIONS/radiance");
            let geo = file.geolocation(&dataset).unwrap().unwrap();
            assert!(geo.latitude.ends_with("/GEODATA/latitude"));
            assert!(geo.latitude_bounds.is_some());
            assert!(geo.longitude_bounds.is_some());
            let shape = file.dataset_info(&dataset).unwrap().shape;
            let map = load_map_data(&file, &dataset, &shape, &geo, Some(0), 248, 160).unwrap();
            assert!(map.points.len() >= 100, "band {band}");
            assert!(map.points.iter().any(|point| point.footprint.is_some()));
            assert!(map
                .points
                .iter()
                .filter_map(|point| point.footprint)
                .any(|corners| {
                    let latitude_span = corners
                        .iter()
                        .map(|corner| corner[1])
                        .fold(f32::NEG_INFINITY, f32::max)
                        - corners
                            .iter()
                            .map(|corner| corner[1])
                            .fold(f32::INFINITY, f32::min);
                    latitude_span > 1.0
                }));
            assert!(map.range.is_some_and(|(min, max)| min < max));
        }
    }

    #[test]
    #[ignore = "manual full-resolution performance measurement"]
    fn benchmark_s5p_full_resolution_map_loading() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mock_datasets/S5P");
        let Some(fixture) = std::fs::read_dir(directory).ok().and_then(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with("S5P_OFFL_L1B_RA_BD3_")
                    })
                })
        }) else {
            return;
        };
        let file = File::open(&fixture).unwrap();
        let dataset = "/BAND3_RADIANCE/STANDARD_MODE/OBSERVATIONS/radiance";
        let geo = file.geolocation(dataset).unwrap().unwrap();
        let shape = file.dataset_info(dataset).unwrap().shape;
        let start = std::time::Instant::now();
        let map = load_map_data(&file, dataset, &shape, &geo, Some(0), 248, usize::MAX).unwrap();
        eprintln!(
            "S5P full map: {:.2}s, {} points",
            start.elapsed().as_secs_f64(),
            map.points.len()
        );
    }

    #[test]
    fn batched_and_row_reads_produce_the_same_map() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let file = File::open(&path).unwrap();
        let dataset = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";
        let geo = file.geolocation(dataset).unwrap().unwrap();
        let shape = file.dataset_info(dataset).unwrap().shape;
        let batched = load_map_data_with_batch_limit(
            &file,
            dataset,
            &shape,
            &geo,
            Some(0),
            32,
            30_000,
            250_000,
        )
        .unwrap();
        let rows =
            load_map_data_with_batch_limit(&file, dataset, &shape, &geo, Some(0), 32, 30_000, 0)
                .unwrap();
        assert_eq!(batched.range, rows.range);
        assert_eq!(batched.points.len(), rows.points.len());
        for (left, right) in batched.points.iter().zip(&rows.points) {
            assert_eq!(left.latitude, right.latitude);
            assert_eq!(left.longitude, right.longitude);
            assert_eq!(left.value.to_bits(), right.value.to_bits());
            assert_eq!(left.special, right.special);
            assert_eq!(left.footprint, right.footprint);
        }
    }

    #[test]
    fn gome_l1b_radiance_maps_one_spectral_channel_when_fixture_is_available() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../PROPOSALS/ER2_RPRO_GOM_L1B____20090606T091303_20090606T093454_73866_02_051000_20171108T192200.nc",
        );
        if !fixture.exists() {
            return;
        }
        let file = File::open(&fixture).unwrap();
        let path = "/MODE_NADIR/BAND_3/OBSERVATIONS/radiance";
        let rule = GeolocationRule {
            file_glob: "ER2_RPRO_GOM_L1B*.nc".into(),
            platform: "ERS-2".into(),
            sensor: "GOME".into(),
            product_type: "GOM_L1B___".into(),
            observation_group: "OBSERVATIONS".into(),
            coordinate_group: "GEODATA".into(),
            latitude: "latitude".into(),
            longitude: "longitude".into(),
            latitude_bounds: "latitude_bounds".into(),
            longitude_bounds: "longitude_bounds".into(),
            scan_direction: None,
        };
        let geo = file.geolocation_with_rules(path, &[rule]).unwrap().unwrap();
        let shape = file.dataset_info(path).unwrap().shape;
        for channel in [0, 1, 100, 500, 900, 1023] {
            let map = load_map_data(&file, path, &shape, &geo, Some(0), channel, 250).unwrap();
            assert_eq!(map.points.len(), 135, "channel {channel}");
            assert!(map.points.iter().any(|point| point.footprint.is_some()));
        }
    }

    #[test]
    #[ignore = "manual performance measurement with local mock dataset"]
    fn benchmark_gome2_map_loading() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../mock_datasets/GOME2/01/12/20130112080756Z_32344_gome.nc");
        if !fixture.exists() {
            return;
        }
        let file = open_dataset_reader(&fixture).unwrap();
        let path = "/MODE_NADIR/BAND_1A/OBSERVATIONS/radiance";
        let settings = Settings::load().unwrap();
        let geo = file
            .geolocation_with_rules(path, &settings.dataset_rules)
            .unwrap()
            .unwrap();
        let shape = file.dataset_info(path).unwrap().shape;
        let start = std::time::Instant::now();
        for _ in 0..5 {
            let map = load_map_data(&file, path, &shape, &geo, Some(0), 0, 30_000).unwrap();
            std::hint::black_box(map.points.len());
        }
        eprintln!(
            "GOME-2 map: {:.2} ms/load",
            start.elapsed().as_secs_f64() * 200.0
        );
    }

    #[test]
    #[ignore = "manual performance measurement with local FDR product"]
    fn benchmark_fdr_multi_time_map_loading() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../mock_datasets/FDR/ESA_FDR_ATMOS__L1B_UVN__20090606_20251029T193846_v01_01.nc",
        );
        if !fixture.exists() {
            return;
        }
        let file = open_dataset_reader(&fixture).unwrap();
        let path = "/GOME/NIR/OBSERVATIONS/radiance_fdr";
        let settings = Settings::load().unwrap();
        let geo = file
            .geolocation_with_rules(path, &settings.dataset_rules)
            .unwrap()
            .unwrap();
        let shape = file.dataset_info(path).unwrap().shape;
        for run in 0..2 {
            let start = std::time::Instant::now();
            let map = load_map_data(&file, path, &shape, &geo, None, 0, 30_000).unwrap();
            eprintln!(
                "FDR multi-time map run {run}: {:.2} ms, {} points",
                start.elapsed().as_secs_f64() * 1_000.0,
                map.points.len()
            );
            std::hint::black_box(map.range);
        }
    }
}
