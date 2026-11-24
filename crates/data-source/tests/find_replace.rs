use h5jive_data::{File, NumericMatch};
use std::sync::atomic::{AtomicBool, AtomicU64};

#[test]
fn searches_every_dimension_and_replaces_large_integers_exactly() {
    let path = std::env::temp_dir().join(format!("h5jive-find-replace-{}.h5", std::process::id()));
    let old = 9_007_199_254_740_993_i64;
    let new = 9_007_199_254_740_995_i64;
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file
            .new_dataset::<i64>()
            .shape([2, 2, 5000])
            .create("values")
            .unwrap();
        let mut values = vec![0_i64; 20_000];
        values[10_000 + 5000 + 4099] = old;
        values[100] = old;
        dataset.write_raw(&values).unwrap();
    }
    let progress = AtomicU64::new(0);
    let cancel = AtomicBool::new(false);
    let file = File::open(&path).unwrap();
    let result = file
        .search_numeric(
            "/values",
            &old.to_string(),
            NumericMatch::Exact,
            0,
            1,
            &progress,
            &cancel,
        )
        .unwrap();
    assert_eq!(result.matches, 2);
    assert_eq!(result.samples[0].coordinates, [0, 0, 100]);
    assert_eq!(progress.load(std::sync::atomic::Ordering::Relaxed), 20_000);
    let page = file
        .search_numeric(
            "/values",
            &old.to_string(),
            NumericMatch::Exact,
            1,
            1,
            &AtomicU64::new(0),
            &cancel,
        )
        .unwrap();
    assert_eq!(page.samples[0].coordinates, [1, 1, 4099]);
    drop(file);
    let outcome = File::replace_numeric_matches(
        &path,
        "/values",
        &old.to_string(),
        &new.to_string(),
        NumericMatch::Exact,
        2,
        &AtomicU64::new(0),
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.replaced, 2);
    let file = File::open(&path).unwrap();
    assert_eq!(
        file.search_numeric(
            "/values",
            &new.to_string(),
            NumericMatch::Exact,
            0,
            3,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        2
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn finds_nan_and_fill_as_distinct_queries() {
    let path = std::env::temp_dir().join(format!("h5jive-find-special-{}.h5", std::process::id()));
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file
            .new_dataset::<f64>()
            .shape([4])
            .create("values")
            .unwrap();
        dataset
            .new_attr::<f64>()
            .create("_FillValue")
            .unwrap()
            .write_scalar(&-9999.0)
            .unwrap();
        dataset
            .write_raw(&[f64::NAN, -9999.0, 1.0, f64::NAN])
            .unwrap();
    }
    let file = File::open(&path).unwrap();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        file.search_numeric(
            "/values",
            "NaN",
            NumericMatch::Exact,
            0,
            4,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        2
    );
    assert_eq!(
        file.search_numeric(
            "/values",
            "Fill",
            NumericMatch::Exact,
            0,
            4,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        1
    );
    drop(file);
    let outcome = File::replace_numeric_matches(
        &path,
        "/values",
        "NaN",
        "Fill",
        NumericMatch::Exact,
        2,
        &AtomicU64::new(0),
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let file = File::open(&path).unwrap();
    assert_eq!(
        file.search_numeric(
            "/values",
            "Fill",
            NumericMatch::Exact,
            0,
            4,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        3
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn matches_within_tolerance_and_rounded_decimals() {
    let path = std::env::temp_dir().join(format!("h5jive-find-fuzzy-{}.h5", std::process::id()));
    {
        let file = hdf5_metno::File::create(&path).unwrap();
        let dataset = file
            .new_dataset::<f64>()
            .shape([4])
            .create("values")
            .unwrap();
        dataset.write_raw(&[1.0, 1.04, 1.06, 2.0]).unwrap();
    }
    let file = File::open(&path).unwrap();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        file.search_numeric(
            "/values",
            "1.0",
            NumericMatch::Tolerance(0.05),
            0,
            8,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        2
    );
    assert_eq!(
        file.search_numeric(
            "/values",
            "1.0",
            NumericMatch::Rounded(1),
            0,
            8,
            &AtomicU64::new(0),
            &cancel
        )
        .unwrap()
        .matches,
        2
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}
