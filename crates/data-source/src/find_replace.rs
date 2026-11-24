//! Bounded whole-dataset numeric search and replacement.
use super::*;
use hdf5_metno::H5Type;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const CHUNK: usize = 4096;

#[derive(Debug, Clone)]
pub struct NumericSearchMatch {
    pub coordinates: Vec<usize>,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct NumericSearchSummary {
    pub matches: u64,
    pub samples: Vec<NumericSearchMatch>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericMatch {
    Exact,
    Tolerance(f64),
    Rounded(u32),
}

#[derive(Debug)]
pub struct NumericReplaceOutcome {
    pub replaced: u64,
    pub error: Option<String>,
}

fn typed_matches<T: Copy + PartialEq>(
    value: T,
    wanted: T,
    needle_is_nan: bool,
    is_nan: impl Fn(T) -> bool,
    to_f64: impl Fn(T) -> f64,
    rule: NumericMatch,
) -> bool {
    if needle_is_nan {
        return is_nan(value);
    }
    match rule {
        NumericMatch::Exact => value == wanted,
        NumericMatch::Tolerance(tolerance) => (to_f64(value) - to_f64(wanted)).abs() <= tolerance,
        NumericMatch::Rounded(decimals) => {
            let factor = 10_f64.powi(decimals as i32);
            (to_f64(value) * factor).round() == (to_f64(wanted) * factor).round()
        }
    }
}

fn scan_typed<T: H5Type + Copy + PartialEq + ToString>(
    dataset: &H5Dataset,
    wanted: T,
    is_nan: impl Fn(T) -> bool,
    to_f64: impl Fn(T) -> f64,
    rule: NumericMatch,
    replacement: Option<T>,
    first_result: u64,
    result_limit: usize,
    progress: &AtomicU64,
    cancel: &AtomicBool,
) -> Result<NumericSearchSummary> {
    let shape = dataset.shape();
    if shape.is_empty() {
        return Err(error("Scalar datasets are not supported by Find / Replace"));
    }
    let width = *shape.last().unwrap();
    let prefix = &shape[..shape.len() - 1];
    let rows = prefix
        .iter()
        .try_fold(1usize, |n, &size| n.checked_mul(size))
        .ok_or_else(|| error("Dataset is too large to index"))?;
    let needle_is_nan = is_nan(wanted);
    let mut result = NumericSearchSummary {
        matches: 0,
        samples: Vec::new(),
    };
    for row in 0..rows {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("Search cancelled"));
        }
        let mut position = vec![0; prefix.len()];
        let mut remaining = row;
        for axis in (0..prefix.len()).rev() {
            position[axis] = remaining % prefix[axis];
            remaining /= prefix[axis];
        }
        for offset in (0..width).step_by(CHUNK) {
            if cancel.load(Ordering::Relaxed) {
                return Err(error("Search cancelled"));
            }
            let length = (width - offset).min(CHUNK);
            let mut start = position.clone();
            start.push(offset);
            let mut count = vec![1; shape.len()];
            *count.last_mut().unwrap() = length;
            let selection = Hyperslab::from(
                start
                    .iter()
                    .zip(&count)
                    .map(|(&s, &n)| SliceOrIndex::from(s..s + n))
                    .collect::<Vec<_>>(),
            );
            let mut values = dataset
                .read_slice::<T, _, IxDyn>(selection.clone())
                .map_err(error)?
                .into_raw_vec_and_offset()
                .0;
            let mut changed = false;
            for (column, value) in values.iter_mut().enumerate() {
                if typed_matches(*value, wanted, needle_is_nan, &is_nan, &to_f64, rule) {
                    if result.matches >= first_result && result.samples.len() < result_limit {
                        let mut coordinates = position.clone();
                        coordinates.push(offset + column);
                        result.samples.push(NumericSearchMatch {
                            coordinates,
                            value: value.to_string(),
                        });
                    }
                    result.matches += 1;
                    if let Some(new_value) = replacement {
                        *value = new_value;
                        changed = true;
                    }
                }
            }
            if changed {
                let array = ArrayD::from_shape_vec(IxDyn(&count), values).map_err(error)?;
                dataset
                    .write_slice(array.view(), selection)
                    .map_err(error)?;
            }
            progress.fetch_add(length as u64, Ordering::Relaxed);
        }
    }
    Ok(result)
}

fn scan_hdf5(
    dataset: &H5Dataset,
    needle: &str,
    replacement: Option<&str>,
    rule: NumericMatch,
    first_result: u64,
    result_limit: usize,
    progress: &AtomicU64,
    cancel: &AtomicBool,
) -> Result<NumericSearchSummary> {
    let descriptor = dataset
        .dtype()
        .map_err(error)?
        .to_descriptor()
        .map_err(error)?;
    let needle = normalize_dataset_numeric_value(dataset, &descriptor, needle)?;
    let replacement = replacement
        .map(|text| normalize_dataset_numeric_value(dataset, &descriptor, text))
        .transpose()?;
    macro_rules! scan {
        ($ty:ty, $nan:expr, $to:expr) => {{
            let wanted = needle.parse::<$ty>().map_err(error)?;
            let new_value = replacement
                .as_ref()
                .map(|s| s.parse::<$ty>().map_err(error))
                .transpose()?;
            scan_typed(
                dataset,
                wanted,
                $nan,
                $to,
                rule,
                new_value,
                first_result,
                result_limit,
                progress,
                cancel,
            )
        }};
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => scan!(i8, |_: i8| false, |v: i8| v as f64),
        TypeDescriptor::Integer(IntSize::U2) => scan!(i16, |_: i16| false, |v: i16| v as f64),
        TypeDescriptor::Integer(IntSize::U4) => scan!(i32, |_: i32| false, |v: i32| v as f64),
        TypeDescriptor::Integer(IntSize::U8) => scan!(i64, |_: i64| false, |v: i64| v as f64),
        TypeDescriptor::Unsigned(IntSize::U1) => scan!(u8, |_: u8| false, |v: u8| v as f64),
        TypeDescriptor::Unsigned(IntSize::U2) => scan!(u16, |_: u16| false, |v: u16| v as f64),
        TypeDescriptor::Unsigned(IntSize::U4) => scan!(u32, |_: u32| false, |v: u32| v as f64),
        TypeDescriptor::Unsigned(IntSize::U8) => scan!(u64, |_: u64| false, |v: u64| v as f64),
        TypeDescriptor::Float(FloatSize::U4) => {
            scan!(f32, |v: f32| v.is_nan(), |v: f32| v as f64)
        }
        TypeDescriptor::Float(FloatSize::U8) => {
            scan!(f64, |v: f64| v.is_nan(), |v: f64| v)
        }
        _ => Err(error(format!(
            "Searching {descriptor} values is not supported"
        ))),
    }
}

impl File {
    pub fn search_numeric(
        &self,
        path: &str,
        needle: &str,
        match_rule: NumericMatch,
        first_result: u64,
        result_limit: usize,
        progress: &AtomicU64,
        cancel: &AtomicBool,
    ) -> Result<NumericSearchSummary> {
        match &self.inner {
            Backend::Hdf5(file) => scan_hdf5(
                &file.dataset(path).map_err(error)?,
                needle,
                None,
                match_rule,
                first_result,
                result_limit,
                progress,
                cancel,
            ),
            Backend::ClassicNetcdf(_) => {
                let shape = self.dataset_info(path)?.shape;
                let wanted = needle.trim().parse::<f64>().map_err(|_| {
                    error("Enter a numeric value or NaN; Fill search requires HDF5 or NetCDF-4")
                })?;
                let mut result = NumericSearchSummary {
                    matches: 0,
                    samples: Vec::new(),
                };
                if shape.is_empty() {
                    return Err(error("Scalar datasets are not supported by Find / Replace"));
                }
                let width = *shape.last().unwrap();
                let prefix = &shape[..shape.len() - 1];
                let rows = prefix
                    .iter()
                    .try_fold(1usize, |n, &size| n.checked_mul(size))
                    .ok_or_else(|| error("Dataset is too large to index"))?;
                for row in 0..rows {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(error("Search cancelled"));
                    }
                    let mut position = vec![0; prefix.len()];
                    let mut remaining = row;
                    for axis in (0..prefix.len()).rev() {
                        position[axis] = remaining % prefix[axis];
                        remaining /= prefix[axis];
                    }
                    for offset in (0..width).step_by(CHUNK) {
                        let length = (width - offset).min(CHUNK);
                        let mut start = position.clone();
                        start.push(offset);
                        let mut count = vec![1; shape.len()];
                        *count.last_mut().unwrap() = length;
                        let values = self.region(path, &start, &count)?.data;
                        for (column, value) in values.into_iter().enumerate() {
                            let value_matches = if wanted.is_nan() {
                                value.is_nan()
                            } else {
                                match match_rule {
                                    NumericMatch::Exact => value == wanted,
                                    NumericMatch::Tolerance(tolerance) => {
                                        (value - wanted).abs() <= tolerance
                                    }
                                    NumericMatch::Rounded(decimals) => {
                                        let factor = 10_f64.powi(decimals as i32);
                                        (value * factor).round() == (wanted * factor).round()
                                    }
                                }
                            };
                            if value_matches {
                                if result.matches >= first_result
                                    && result.samples.len() < result_limit
                                {
                                    let mut coordinates = position.clone();
                                    coordinates.push(offset + column);
                                    result.samples.push(NumericSearchMatch {
                                        coordinates,
                                        value: value.to_string(),
                                    });
                                }
                                result.matches += 1;
                            }
                        }
                        progress.fetch_add(length as u64, Ordering::Relaxed);
                    }
                }
                Ok(result)
            }
            Backend::Unavailable => Err(error("File is being updated")),
        }
    }

    pub fn replace_numeric_matches(
        path: &Path,
        dataset_path: &str,
        needle: &str,
        replacement: &str,
        match_rule: NumericMatch,
        expected_count: u64,
        progress: &AtomicU64,
    ) -> NumericReplaceOutcome {
        let fail = |message: String| NumericReplaceOutcome {
            replaced: 0,
            error: Some(message),
        };
        let file = match WritableFile::open(path) {
            Ok(file) => file,
            Err(err) => return fail(err.to_string()),
        };
        let dataset = match file.dataset(dataset_path) {
            Ok(dataset) => dataset,
            Err(err) => return fail(err.to_string()),
        };
        let cancel = AtomicBool::new(false);
        // Count again while holding the write lock. A changed source must be previewed again.
        let counted = match scan_hdf5(
            &dataset,
            needle,
            None,
            match_rule,
            0,
            0,
            &AtomicU64::new(0),
            &cancel,
        ) {
            Ok(summary) => summary.matches,
            Err(err) => return fail(err.to_string()),
        };
        if counted != expected_count {
            return fail(format!(
                "Match count changed from {expected_count} to {counted}; run Find again"
            ));
        }
        let summary = match scan_hdf5(
            &dataset,
            needle,
            Some(replacement),
            match_rule,
            0,
            0,
            progress,
            &cancel,
        ) {
            Ok(summary) => summary,
            Err(err) => return fail(err.to_string()),
        };
        if let Err(err) = file.flush() {
            return NumericReplaceOutcome {
                replaced: summary.matches,
                error: Some(err.to_string()),
            };
        }
        NumericReplaceOutcome {
            replaced: summary.matches,
            error: None,
        }
    }
}
