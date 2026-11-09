use super::super::super::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_includes_both_ends_without_duplicates() {
        assert_eq!(sampled_indexes(48, 5), [0, 11, 23, 35, 47]);
        assert_eq!(sampled_indexes(3, 20), [0, 1, 2]);
    }

    #[test]
    fn gome2_example_has_values_and_footprints_in_both_formats() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        for (file_name, path) in [
            ("space_demo.hdf5", "/satellite/gome2/so2_slant_column"),
            ("space_observation.nc", "/so2_slant_column"),
        ] {
            let file = File::open(&root.join(file_name)).unwrap();
            let geo = file.geolocation(path).unwrap().unwrap();
            let shape = file.dataset_info(path).unwrap().shape;
            let all = load_map_data(&file, path, &shape, &geo, None, 0, 30_000).unwrap();
            assert_eq!(all.points.len(), shape.iter().product::<usize>());
            assert!(all.points.iter().all(|point| point.footprint.is_some()));
            assert!(all
                .points
                .iter()
                .all(|point| (-90.0..=90.0).contains(&point.latitude)));
            assert!(all.range.is_some_and(|(min, max)| min < max));
            assert!(all.points.iter().any(|point| point.special.is_some()));
        }
    }

    #[test]
    fn batched_and_row_reads_produce_the_same_map() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/space_observation.nc");
        let file = File::open(&path).unwrap();
        let dataset = "/so2_slant_column";
        let geo = file.geolocation(dataset).unwrap().unwrap();
        let shape = file.dataset_info(dataset).unwrap().shape;
        let batched =
            load_map_data_with_batch_limit(&file, dataset, &shape, &geo, None, 0, 30_000, 250_000)
                .unwrap();
        let rows = load_map_data_with_batch_limit(&file, dataset, &shape, &geo, None, 0, 30_000, 0)
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
