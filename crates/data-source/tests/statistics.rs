use h5jive_data::File;
use hdf5_metno::{
    types::{FixedAscii, VarLenUnicode},
    File as H5File,
};
use std::{path::PathBuf, str::FromStr};

#[test]
fn exact_quartiles_outlier_and_group_attribute() {
    let path: PathBuf =
        std::env::temp_dir().join(format!("h5jive-stats-{}.hdf5", std::process::id()));
    {
        let hdf5 = H5File::create(&path).unwrap();
        let group = hdf5.create_group("measurements").unwrap();
        let title = VarLenUnicode::from_str("regional measurements").unwrap();
        group
            .new_attr::<VarLenUnicode>()
            .create("title")
            .unwrap()
            .write_scalar(&title)
            .unwrap();
        let fixed = FixedAscii::<16>::from_ascii("test station").unwrap();
        group
            .new_attr::<FixedAscii<16>>()
            .create("station")
            .unwrap()
            .write_scalar(&fixed)
            .unwrap();
        group
            .new_attr::<i64>()
            .create("identifier")
            .unwrap()
            .write_scalar(&9_000_000_000_000_001_i64)
            .unwrap();
        group
            .new_dataset::<f64>()
            .shape([5])
            .create("sample")
            .unwrap()
            .write_raw(&[1.0, 2.0, 2.0, 3.0, 100.0])
            .unwrap();
        group
            .new_dataset::<f64>()
            .shape([2, 3])
            .create("plain_2d")
            .unwrap()
            .write_raw(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0])
            .unwrap();
        let special = group
            .new_dataset::<f64>()
            .shape([7])
            .create("special")
            .unwrap();
        special
            .write_raw(&[
                -2.0,
                0.0,
                2.0,
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                -9999.0,
            ])
            .unwrap();
        special
            .new_attr::<f64>()
            .create("_FillValue")
            .unwrap()
            .write_scalar(&-9999.0)
            .unwrap();
        let array_fill = group
            .new_dataset::<f32>()
            .shape([2])
            .create("array_fill")
            .unwrap();
        array_fill.write_raw(&[1.0, -9999.0]).unwrap();
        array_fill
            .new_attr::<f32>()
            .shape([1])
            .create("_FillValue")
            .unwrap()
            .write_raw(&[-9999.0])
            .unwrap();
    }
    let file = File::open(&path).unwrap();
    assert_eq!(
        file.axis_names("/measurements/plain_2d").unwrap(),
        ["Dimension 0", "Dimension 1"]
    );
    let axes = file.coordinate_axes("/measurements/plain_2d").unwrap();
    assert_eq!(
        axes.iter().map(|axis| axis.length).collect::<Vec<_>>(),
        [2, 3]
    );
    assert!(axes.iter().all(|axis| axis.variable.is_none()));
    let attributes = file.attributes("/measurements").unwrap();
    assert!(attributes
        .iter()
        .any(|attribute| attribute.name == "title" && attribute.value == "regional measurements"));
    assert!(
        attributes
            .iter()
            .any(|attribute| attribute.name == "station" && attribute.value == "test station"),
        "{attributes:?}"
    );
    assert!(attributes
        .iter()
        .any(|attribute| attribute.name == "identifier" && attribute.value == "9000000000000001"));
    let statistics = file.statistics("/measurements/sample", 10).unwrap();
    assert_eq!(statistics.median, Some(2.0));
    assert_eq!(statistics.first_quartile, Some(2.0));
    assert_eq!(statistics.third_quartile, Some(3.0));
    assert_eq!(statistics.mean, Some(21.6));
    assert_eq!(statistics.outliers, 1);
    assert_eq!(statistics.positive, 5);
    let distribution = file
        .distribution("/measurements/sample", &statistics, 32)
        .unwrap();
    assert_eq!(distribution.bins.iter().sum::<u128>(), statistics.valid);
    assert_eq!(distribution.lower_whisker, 1.0);
    assert_eq!(distribution.upper_whisker, 3.0);
    let sampled = file.statistics("/measurements/sample", 2).unwrap();
    assert!(sampled.approximate_quartiles);
    assert_eq!(sampled.valid, 5);
    let special = file.statistics("/measurements/special", 10).unwrap();
    assert_eq!(
        (special.positive, special.zero, special.negative),
        (1, 1, 1)
    );
    assert_eq!(
        (
            special.nan,
            special.positive_infinity,
            special.negative_infinity,
            special.fill
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(special.valid, 3);
    let distribution = file
        .distribution("/measurements/special", &special, 32)
        .unwrap();
    assert_eq!(distribution.bins.iter().sum::<u128>(), 3);
    let array_fill_stats = file.statistics("/measurements/array_fill", 10).unwrap();
    assert_eq!(array_fill_stats.fill, 1);
    assert_eq!(array_fill_stats.valid, 1);
    let array_fill_distribution = file
        .distribution("/measurements/array_fill", &array_fill_stats, 16)
        .unwrap();
    assert_eq!(array_fill_distribution.bins.iter().sum::<u128>(), 1);
    assert_eq!(
        file.region("/measurements/array_fill", &[0], &[2])
            .unwrap()
            .fill_value,
        Some(-9999.0)
    );
    assert_eq!(
        file.values("/measurements/array_fill", 2)
            .unwrap()
            .fill_value,
        Some(-9999.0)
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}
