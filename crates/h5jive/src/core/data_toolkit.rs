//! Shared dataset selection, value classification, formatting, and coordinate routines.
use super::*;

pub(crate) fn pinned_indices(shape: &[usize], default: PinnedDefault) -> Vec<usize> {
    shape.iter().map(|&length| default.index(length)).collect()
}

/// Build a hyperslab from fixed indexes and overrides for its varying axes.
pub(crate) fn region_from_pins(
    pinned: &[usize],
    varying: &[(usize, usize, usize)],
) -> (Vec<usize>, Vec<usize>) {
    let mut start = pinned.to_vec();
    let mut count = vec![1; pinned.len()];
    for &(axis, first, length) in varying {
        start[axis] = first;
        count[axis] = length;
    }
    (start, count)
}

pub(crate) fn axis_ticks(length: usize, maximum: usize) -> Vec<usize> {
    let count = length.min(maximum.max(1));
    if count == 1 {
        return vec![0];
    }
    (0..count)
        .map(|position| ((position as u128 * (length - 1) as u128) / (count - 1) as u128) as usize)
        .collect()
}

pub(crate) fn axis_center(index: usize, length: usize) -> f32 {
    ((index as f64 + 0.5) / length as f64 - 0.5) as f32
}

pub(crate) fn axis_option_labels(names: &[String], shape: &[usize]) -> Vec<String> {
    shape
        .iter()
        .enumerate()
        .map(|(axis, length)| format!("{} ({length})", axis_name(names, axis)))
        .collect()
}

pub(crate) fn open_dataset_reader(path: &std::path::Path) -> Result<File, String> {
    File::open_with_options(
        path,
        OpenOptions {
            max_depth: 1,
            max_objects: 1,
        },
    )
    .map_err(|err| err.to_string())
}

fn trim_decimal(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}
pub(crate) fn format_value(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "+Inf".into()
    } else if value == f64::NEG_INFINITY {
        "-Inf".into()
    } else if value == 0.0 {
        "0".into()
    } else if (0.001..=1000.0).contains(&value.abs()) {
        trim_decimal(&format!("{value:.decimals$}")).to_owned()
    } else {
        let scientific = format!("{value:.decimals$e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        format!("{}e{exponent}", trim_decimal(mantissa))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpecialValue {
    Fill,
    Nan,
    PositiveInf,
    NegativeInf,
}
impl SpecialValue {
    pub(crate) fn from_value(value: f64, fill: Option<f64>) -> Option<Self> {
        if fill == Some(value) {
            Some(Self::Fill)
        } else if value.is_nan() {
            Some(Self::Nan)
        } else if value == f64::INFINITY {
            Some(Self::PositiveInf)
        } else if value == f64::NEG_INFINITY {
            Some(Self::NegativeInf)
        } else {
            None
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Nan => "NaN",
            Self::PositiveInf => "+Inf",
            Self::NegativeInf => "-Inf",
        }
    }
}
pub(crate) fn tick_coordinate<'a>(index: usize, label: &'a str) -> Option<&'a str> {
    label.strip_prefix(&format!("{index} ("))?.strip_suffix(')')
}
pub(crate) fn axis_name(names: &[String], axis: usize) -> String {
    names
        .get(axis)
        .filter(|name| !name.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("Dimension {axis}"))
}
pub(crate) fn format_stat(value: Option<f64>, decimals: usize) -> String {
    value
        .map(|number| format_value(number, decimals))
        .unwrap_or_else(|| "-".into())
}
pub(crate) fn format_attribute_value(attribute: &AttributeValue, decimals: usize) -> String {
    let Some(values) = &attribute.float_values else {
        return attribute.value.clone();
    };
    if values.len() == 1 {
        format_value(values[0], decimals)
    } else {
        let shown = values
            .iter()
            .take(64)
            .map(|&value| format_value(value, decimals))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{shown}{}]", if values.len() > 64 { ", ..." } else { "" })
    }
}
pub(crate) fn human_bytes(bytes: u128, decimals: usize) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{} {}", format_value(value, decimals), units[unit])
    }
}

/// A value is usable for numeric ranges and profiles when it is finite and not fill.
pub(crate) fn finite_value(value: f64, fill: Option<f64>) -> Option<f64> {
    SpecialValue::from_value(value, fill)
        .is_none()
        .then_some(value)
}

pub(crate) fn extend_finite_range(
    range: &mut Option<(f64, f64)>,
    values: &[f64],
    fill: Option<f64>,
) {
    for &value in values {
        let Some(value) = finite_value(value, fill) else {
            continue;
        };
        *range = Some(match *range {
            Some((min, max)) => (min.min(value), max.max(value)),
            None => (value, value),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_hyperslab_overrides_only_varying_axes() {
        let (start, count) = region_from_pins(&[2, 3, 4, 5], &[(1, 7, 8), (3, 9, 10)]);
        assert_eq!(start, [2, 7, 4, 9]);
        assert_eq!(count, [1, 8, 1, 10]);
    }

    #[test]
    fn index_positions_are_at_voxel_centers() {
        assert!((axis_center(0, 10) + 0.45).abs() < 1e-6);
        assert!((axis_center(9, 10) - 0.45).abs() < 1e-6);
    }
}
