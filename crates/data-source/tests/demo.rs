use h5jive_data::{Entry, File, Kind};
use std::path::Path;

fn find<'a>(entry: &'a Entry, path: &str) -> &'a Entry {
    if entry.path == path {
        return entry;
    }
    entry
        .children
        .iter()
        .find_map(|child| {
            if child.path == path || path.starts_with(&format!("{}/", child.path)) {
                Some(find(child, path))
            } else {
                None
            }
        })
        .expect("dataset path in demo tree")
}

#[test]
fn demo_opens_with_expected_axes_and_sentinel_regions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).expect("open committed demo");
    assert!(file
        .tree
        .children
        .iter()
        .all(|entry| entry.name != "satellite"));
    let info = file.dataset_info("/observations/swath_2d").unwrap();
    assert_eq!(info.shape, [32, 48]);
    assert_eq!(info.elements, 1536);
    assert_eq!(info.byte_size, 12_288);
    assert!(info.numeric);
    let attributes = file.attributes("/observations/swath_2d").unwrap();
    assert!(attributes
        .iter()
        .any(|attribute| attribute.name == "coordinates"
            && attribute.value == "scanline ground_pixel"));
    assert!(attributes
        .iter()
        .any(|attribute| attribute.name == "_FillValue" && attribute.value == "-9999"));
    assert!(attributes
        .iter()
        .any(|attribute| attribute.name == "_FillValue"
            && attribute.float_values.as_deref() == Some(&[-9999.0][..])));
    assert!(file.attributes("/observations").unwrap().is_empty());
    let statistics = file
        .statistics("/observations/swath_2d", 1_000_000)
        .unwrap();
    assert_eq!(statistics.nan, 35);
    assert_eq!(statistics.positive_infinity, 35);
    assert_eq!(statistics.negative_infinity, 35);
    assert_eq!(statistics.fill, 35);
    assert_eq!(
        statistics.valid
            + statistics.nan
            + statistics.positive_infinity
            + statistics.negative_infinity
            + statistics.fill,
        1536
    );
    assert!(!statistics.approximate_quartiles);
    assert_eq!(
        file.axis_names("/observations/swath_2d").unwrap(),
        ["scanline", "ground_pixel"]
    );
    assert_eq!(
        file.axis_names("/observations/time_spectral_cube_4d")
            .unwrap(),
        ["time", "scanline", "ground_pixel", "wavelength"]
    );
    let axes = file.coordinate_axes("/observations/swath_2d").unwrap();
    assert_eq!(axes.len(), 2);
    assert_eq!(axes[0].name, "scanline");
    assert_eq!(axes[0].length, 32);
    assert_eq!(axes[0].variable.as_deref(), Some("/coordinates/scanline"));
    assert_eq!(axes[0].units.as_deref(), Some("km"));
    assert_eq!(axes[0].first, Some(-12.0));
    assert_eq!(axes[0].last, Some(12.0));
    for (name, len) in [
        ("time", 8),
        ("scanline", 32),
        ("ground_pixel", 48),
        ("wavelength", 16),
    ] {
        let path = format!("/coordinates/{name}");
        let axis = find(&file.tree, &path);
        assert!(matches!(&axis.kind, Kind::Dataset { shape, .. } if shape == &[len]));
        let values = file.values(&path, 1_000_000).unwrap();
        assert!(values.data.windows(2).all(|pair| pair[0] < pair[1]));
    }
    for (name, shape, count) in [
        ("swath_2d", vec![32, 48], 35),
        ("spectral_cube_3d", vec![32, 48, 16], 119),
        ("time_spectral_cube_4d", vec![8, 32, 48, 16], 451),
    ] {
        let path = format!("/observations/{name}");
        let entry = find(&file.tree, &path);
        assert!(
            matches!(&entry.kind, Kind::Dataset { shape: actual, numeric: true } if actual == &shape)
        );
        let values = file.values(&path, 1_000_000).unwrap();
        assert_eq!(values.fill_value, Some(-9999.0));
        assert_eq!(values.data.iter().filter(|x| x.is_nan()).count(), count);
        assert_eq!(
            values.data.iter().filter(|&&x| x == f64::INFINITY).count(),
            count
        );
        assert_eq!(
            values
                .data
                .iter()
                .filter(|&&x| x == f64::NEG_INFINITY)
                .count(),
            count
        );
        assert_eq!(values.data.iter().filter(|&&x| x == -9999.0).count(), count);
        assert!(values
            .data
            .iter()
            .any(|&x| x.is_finite() && x != -9999.0 && x < 0.0));
        assert!(values
            .data
            .iter()
            .any(|&x| x.is_finite() && x != -9999.0 && x > 0.0));
    }
    let curve = file.values("/observations/time_curve_1d", 200_000).unwrap();
    assert!(curve.data.iter().any(|&x| x < 0.0));
    assert!(curve.data.iter().any(|&x| x > 0.0));
    let cube = file
        .statistics("/observations/time_spectral_cube_4d", 1_000_000)
        .unwrap();
    assert_eq!(cube.info.elements, 8 * 32 * 48 * 16);
    assert_eq!(cube.info.byte_size, 8 * 32 * 48 * 16 * 8);
    assert_eq!(
        (
            cube.nan,
            cube.positive_infinity,
            cube.negative_infinity,
            cube.fill
        ),
        (451, 451, 451, 451)
    );
    assert!(!cube.approximate_quartiles);
}

#[test]
fn reads_only_requested_four_dimensional_region() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/space_demo.hdf5");
    let file = File::open(&path).unwrap();
    let dataset = "/observations/time_spectral_cube_4d";
    let region = file.region(dataset, &[2, 4, 7, 3], &[1, 2, 3, 1]).unwrap();
    assert_eq!(region.shape, [1, 2, 3, 1]);
    assert_eq!(region.fill_value, Some(-9999.0));
    assert_eq!(region.data.len(), 6);
    let all = file.values(dataset, 1_000_000).unwrap();
    for row in 0..2 {
        for column in 0..3 {
            let offset = (((2 * 32 + 4 + row) * 48 + 7 + column) * 16) + 3;
            let expected = all.data[offset];
            let actual = region.data[row * 3 + column];
            assert!(actual == expected || actual.is_nan() && expected.is_nan());
        }
    }
}
