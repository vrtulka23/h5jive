use super::{DatasetInfo, File, Result};

#[derive(Debug, Clone)]
pub struct DatasetStatistics {
    pub info: DatasetInfo,
    pub valid: u128,
    pub positive: u128,
    pub zero: u128,
    pub negative: u128,
    pub nan: u128,
    pub positive_infinity: u128,
    pub negative_infinity: u128,
    pub fill: u128,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub variance: Option<f64>,
    pub standard_deviation: Option<f64>,
    pub median: Option<f64>,
    pub first_quartile: Option<f64>,
    pub third_quartile: Option<f64>,
    pub outliers: u128,
    pub approximate_quartiles: bool,
}

#[derive(Debug, Clone)]
pub struct DatasetDistribution {
    pub bins: Vec<u128>,
    pub lower_whisker: f64,
    pub upper_whisker: f64,
}

pub(super) fn distribution(
    file: &File,
    path: &str,
    statistics: &DatasetStatistics,
    bin_count: usize,
) -> Result<DatasetDistribution> {
    if bin_count == 0 {
        return Err(super::error("histogram bin count must be positive"));
    }
    let mut bins = vec![0_u128; bin_count];
    let (Some(min), Some(max), Some(q1), Some(q3)) = (
        statistics.min,
        statistics.max,
        statistics.first_quartile,
        statistics.third_quartile,
    ) else {
        return Ok(DatasetDistribution {
            bins,
            lower_whisker: 0.0,
            upper_whisker: 0.0,
        });
    };
    let iqr = q3 - q1;
    let lower_fence = q1 - 1.5 * iqr;
    let upper_fence = q3 + 1.5 * iqr;
    let mut lower_whisker = None::<f64>;
    let mut upper_whisker = None::<f64>;
    for_each_value(file, path, &statistics.info.shape, |value, fill_value| {
        if !value.is_finite() || fill_value.is_some_and(|marker| value == marker) {
            return;
        }
        let fraction = if min == max {
            0.5
        } else if (max - min).is_finite() {
            (value - min) / (max - min)
        } else {
            (value * 0.5 - min * 0.5) / (max * 0.5 - min * 0.5)
        };
        let bin = ((fraction.clamp(0.0, 1.0) * bin_count as f64) as usize).min(bin_count - 1);
        bins[bin] += 1;
        if value >= lower_fence && value <= upper_fence {
            lower_whisker = Some(lower_whisker.map_or(value, |current| current.min(value)));
            upper_whisker = Some(upper_whisker.map_or(value, |current| current.max(value)));
        }
    })?;
    Ok(DatasetDistribution {
        bins,
        lower_whisker: lower_whisker.unwrap_or(min),
        upper_whisker: upper_whisker.unwrap_or(max),
    })
}

pub(super) fn calculate(file: &File, path: &str, sample_limit: usize) -> Result<DatasetStatistics> {
    let info = file.dataset_info(path)?;
    let mut result = DatasetStatistics {
        info,
        valid: 0,
        positive: 0,
        zero: 0,
        negative: 0,
        nan: 0,
        positive_infinity: 0,
        negative_infinity: 0,
        fill: 0,
        min: None,
        max: None,
        mean: None,
        variance: None,
        standard_deviation: None,
        median: None,
        first_quartile: None,
        third_quartile: None,
        outliers: 0,
        approximate_quartiles: false,
    };
    if !result.info.numeric || result.info.elements == 0 {
        return Ok(result);
    }
    let limit = sample_limit.max(1);
    let mut sample = Vec::with_capacity((result.info.elements.min(limit as u128)) as usize);
    let mut random_state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut squared_deviations = 0.0_f64;
    for_each_value(file, path, &result.info.shape, |value, fill_value| {
        if value.is_nan() {
            result.nan += 1;
        } else if value == f64::INFINITY {
            result.positive_infinity += 1;
        } else if value == f64::NEG_INFINITY {
            result.negative_infinity += 1;
        } else if fill_value.is_some_and(|marker| value == marker) {
            result.fill += 1;
        } else {
            result.valid += 1;
            if value > 0.0 {
                result.positive += 1;
            } else if value < 0.0 {
                result.negative += 1;
            } else {
                result.zero += 1;
            }
            result.min = Some(result.min.map_or(value, |current| current.min(value)));
            result.max = Some(result.max.map_or(value, |current| current.max(value)));
            let mean = result.mean.get_or_insert(0.0);
            let delta = value - *mean;
            *mean += delta / result.valid as f64;
            squared_deviations += delta * (value - *mean);
            if sample.len() < limit {
                sample.push(value);
            } else {
                random_state ^= random_state << 13;
                random_state ^= random_state >> 7;
                random_state ^= random_state << 17;
                let position = random_state as u128 % result.valid;
                if position < limit as u128 {
                    sample[position as usize] = value;
                }
            }
        }
    })?;
    if result.valid > 0 {
        let variance = (squared_deviations / result.valid as f64).max(0.0);
        result.variance = Some(variance);
        result.standard_deviation = Some(variance.sqrt());
    }
    if sample.is_empty() {
        return Ok(result);
    }
    sample.sort_by(f64::total_cmp);
    result.approximate_quartiles = result.valid > sample.len() as u128;
    let q1 = percentile(&sample, 0.25);
    let q2 = percentile(&sample, 0.5);
    let q3 = percentile(&sample, 0.75);
    result.first_quartile = Some(q1);
    result.median = Some(q2);
    result.third_quartile = Some(q3);
    if result.valid >= 4 {
        let range = q3 - q1;
        let lower = q1 - 1.5 * range;
        let upper = q3 + 1.5 * range;
        if result.approximate_quartiles {
            for_each_value(file, path, &result.info.shape, |value, fill_value| {
                if value.is_finite()
                    && !fill_value.is_some_and(|marker| value == marker)
                    && (value < lower || value > upper)
                {
                    result.outliers += 1;
                }
            })?;
        } else {
            result.outliers = sample
                .iter()
                .filter(|&&value| value < lower || value > upper)
                .count() as u128;
        }
    }
    Ok(result)
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let position = fraction * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
    }
}

fn for_each_value(
    file: &File,
    path: &str,
    shape: &[usize],
    mut visit: impl FnMut(f64, Option<f64>),
) -> Result<()> {
    if shape.iter().any(|&length| length == 0) {
        return Ok(());
    }
    if shape.is_empty() {
        let values = file.values(path, 1)?;
        for value in values.data {
            visit(value, values.fill_value);
        }
        return Ok(());
    }
    // Read larger slabs so compressed HDF5 chunks are decompressed fewer times.
    // File::region caps each request at one million values.
    let mut capacity = 1_000_000_usize;
    let mut block = vec![1; shape.len()];
    for axis in (0..shape.len()).rev() {
        block[axis] = shape[axis].min(capacity.max(1));
        capacity = (capacity / block[axis]).max(1);
    }
    let mut start = vec![0; shape.len()];
    loop {
        let count: Vec<usize> = shape
            .iter()
            .zip(&start)
            .zip(&block)
            .map(|((&length, &first), &step)| (length - first).min(step))
            .collect();
        let region = file.region(path, &start, &count)?;
        for value in region.data {
            visit(value, region.fill_value);
        }
        let mut exhausted = true;
        for axis in (0..shape.len()).rev() {
            start[axis] += block[axis];
            if start[axis] < shape[axis] {
                exhausted = false;
                break;
            }
            start[axis] = 0;
        }
        if exhausted {
            break;
        }
    }
    Ok(())
}
