use h5jive_data::{File, NumericEdit};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_file() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("h5jive-edit-{}-{stamp}.h5", std::process::id()))
}

#[test]
fn staged_integer_cells_write_exact_values_only_on_apply() {
    let path = temporary_file();
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file
            .new_dataset::<i64>()
            .shape([2, 2, 2])
            .create("values")
            .unwrap();
        dataset
            .write_raw(&[9_007_199_254_740_993_i64, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
    }
    let mut file = File::open(&path).unwrap();
    let original = file.numeric_cell("/values", &[0, 0, 0]).unwrap();
    assert_eq!(original, "9007199254740993");
    assert!(file.validate_numeric_edit("/values", "2.5").is_err());
    assert!(file.validate_numeric_edit("/values", "Fill").is_err());
    assert_eq!(
        file.validate_numeric_edit("/values", "9007199254740994")
            .unwrap(),
        "9007199254740994"
    );
    assert_eq!(file.numeric_cell("/values", &[0, 0, 0]).unwrap(), original);
    file.release_for_edit().unwrap();
    let outcome = File::apply_numeric_edits(
        &path,
        "/values",
        &[
            NumericEdit {
                coordinates: vec![0, 0, 0],
                original,
                replacement: "9007199254740994".into(),
            },
            NumericEdit {
                coordinates: vec![1, 1, 1],
                original: "8".into(),
                replacement: "12".into(),
            },
        ],
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.applied, 2);
    let reopened = hdf5_metno::File::open(&path).unwrap();
    let values = reopened
        .dataset("/values")
        .unwrap()
        .read_raw::<i64>()
        .unwrap();
    assert_eq!(values, [9_007_199_254_740_994, 2, 3, 4, 5, 6, 7, 12]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn changed_original_prevents_the_whole_batch() {
    let path = temporary_file();
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        file.new_dataset::<f64>()
            .shape([2])
            .create("x")
            .unwrap()
            .write_raw(&[1.0, 2.0])
            .unwrap();
    }
    let outcome = File::apply_numeric_edits(
        &path,
        "/x",
        &[
            NumericEdit {
                coordinates: vec![0],
                original: "1".into(),
                replacement: "5".into(),
            },
            NumericEdit {
                coordinates: vec![1],
                original: "99".into(),
                replacement: "6".into(),
            },
        ],
    );
    assert_eq!(outcome.applied, 0);
    assert!(outcome.error.unwrap().contains("changed on disk"));
    let file = hdf5_metno::File::open(&path).unwrap();
    assert_eq!(
        file.dataset("/x").unwrap().read_raw::<f64>().unwrap(),
        [1.0, 2.0]
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn special_float_cells_can_be_replaced() {
    let path = temporary_file();
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        file.new_dataset::<f32>()
            .shape([2])
            .create("x")
            .unwrap()
            .write_raw(&[f32::NAN, -9999.0])
            .unwrap();
    }
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.numeric_cell("/x", &[0]).unwrap(), "NaN");
    assert_eq!(file.numeric_cell("/x", &[1]).unwrap(), "-9999");
    assert_eq!(file.validate_numeric_edit("/x", "+Inf").unwrap(), "inf");
    file.release_for_edit().unwrap();
    let outcome = File::apply_numeric_edits(
        &path,
        "/x",
        &[
            NumericEdit {
                coordinates: vec![0],
                original: "NaN".into(),
                replacement: "3.25".into(),
            },
            NumericEdit {
                coordinates: vec![1],
                original: "-9999".into(),
                replacement: "4.5".into(),
            },
        ],
    );
    assert_eq!(outcome.applied, 2);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let reopened = hdf5_metno::File::open(&path).unwrap();
    assert_eq!(
        reopened.dataset("/x").unwrap().read_raw::<f32>().unwrap(),
        [3.25, 4.5]
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn special_text_sets_fill_nan_and_infinities() {
    let path = temporary_file();
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file.new_dataset::<f64>().shape([4]).create("x").unwrap();
        dataset
            .new_attr::<f64>()
            .create("_FillValue")
            .unwrap()
            .write_scalar(&-9999.0)
            .unwrap();
        dataset.write_raw(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    }
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.validate_numeric_edit("/x", "Fill").unwrap(), "-9999");
    assert_eq!(file.validate_numeric_edit("/x", "fill").unwrap(), "-9999");
    assert_eq!(file.validate_numeric_edit("/x", "NaN").unwrap(), "NaN");
    assert_eq!(file.validate_numeric_edit("/x", "nan").unwrap(), "NaN");
    assert_eq!(file.validate_numeric_edit("/x", "+Inf").unwrap(), "inf");
    assert_eq!(file.validate_numeric_edit("/x", "-inf").unwrap(), "-inf");
    file.release_for_edit().unwrap();
    let outcome = File::apply_numeric_edits(
        &path,
        "/x",
        &[
            NumericEdit {
                coordinates: vec![0],
                original: "1".into(),
                replacement: "Fill".into(),
            },
            NumericEdit {
                coordinates: vec![1],
                original: "2".into(),
                replacement: "nan".into(),
            },
            NumericEdit {
                coordinates: vec![2],
                original: "3".into(),
                replacement: "+Inf".into(),
            },
            NumericEdit {
                coordinates: vec![3],
                original: "4".into(),
                replacement: "-Inf".into(),
            },
        ],
    );
    assert_eq!(outcome.applied, 4);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let reopened = hdf5_metno::File::open(&path).unwrap();
    let values = reopened.dataset("/x").unwrap().read_raw::<f64>().unwrap();
    assert_eq!(values[0], -9999.0);
    assert!(values[1].is_nan());
    assert_eq!(values[2], f64::INFINITY);
    assert_eq!(values[3], f64::NEG_INFINITY);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn integer_fill_text_preserves_the_exact_sentinel() {
    let path = temporary_file();
    let fill = 9_007_199_254_740_993_i64;
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file.new_dataset::<i64>().shape([1]).create("x").unwrap();
        dataset
            .new_attr::<i64>()
            .create("_FillValue")
            .unwrap()
            .write_scalar(&fill)
            .unwrap();
        dataset.write_raw(&[0_i64]).unwrap();
    }
    let mut file = File::open(&path).unwrap();
    assert_eq!(
        file.validate_numeric_edit("/x", "fill").unwrap(),
        fill.to_string()
    );
    assert!(file.validate_numeric_edit("/x", "nan").is_err());
    assert!(file.validate_numeric_edit("/x", "+Inf").is_err());
    file.release_for_edit().unwrap();
    let outcome = File::apply_numeric_edits(
        &path,
        "/x",
        &[NumericEdit {
            coordinates: vec![0],
            original: "0".into(),
            replacement: "Fill".into(),
        }],
    );
    assert_eq!(outcome.applied, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let reopened = hdf5_metno::File::open(&path).unwrap();
    assert_eq!(
        reopened.dataset("/x").unwrap().read_raw::<i64>().unwrap(),
        [fill]
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
