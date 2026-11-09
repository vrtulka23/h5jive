//! Volume axis selection and exact data loading.
use super::*;

pub(crate) fn assign_volume_axis(slice: &mut VolumeSlice, slot: usize, axis: usize) {
    let visible_count = slice.visible_count();
    let mut slots: Vec<usize> = slice.axes.iter().flatten().copied().collect();
    if let Some(hidden) = slice.hidden {
        slots.push(hidden);
    }
    if let Some(other) = slots.iter().position(|entry| *entry == axis) {
        slots.swap(slot, other);
        for (index, entry) in slice.axes.iter_mut().enumerate() {
            *entry = slots.get(index).filter(|_| index < visible_count).copied();
        }
        if slice.hidden.is_some() {
            slice.hidden = slots.get(visible_count).copied();
        }
    }
}

/// Match each displayed side to its native number of index pixels.
pub(crate) fn volume_axis_ratios(slice: &VolumeSlice, shape: &[usize]) -> [f32; 3] {
    let longest = slice
        .axes
        .iter()
        .flatten()
        .map(|&axis| shape[axis])
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    std::array::from_fn(|slot| {
        slice.axes[slot].map_or(1.0, |axis| shape[axis].max(1) as f32 / longest)
    })
}

/// Center of the displayed voxel bounds in the renderer's axis coordinates.
pub(crate) fn volume_visible_center(slice: &VolumeSlice, shape: &[usize]) -> [f32; 3] {
    std::array::from_fn(|slot| {
        let Some(axis) = slice.axes[slot] else {
            return 0.0;
        };
        let [first, last] = slice.ranges[axis];
        0.5 - (first as f32 + last as f32 + 1.0) / (2.0 * shape[axis] as f32)
    })
}

pub(crate) fn volume_visible_bounds(slice: &VolumeSlice, shape: &[usize]) -> [[f32; 2]; 3] {
    std::array::from_fn(|slot| {
        slice.axes[slot].map_or([-0.04, 0.04], |axis| {
            let [first, last] = slice.ranges[axis];
            [
                0.5 - (last + 1) as f32 / shape[axis] as f32,
                0.5 - first as f32 / shape[axis] as f32,
            ]
        })
    })
}

/// Use the central value distribution for automatic colors while retaining
/// the exact extrema separately. A bounded, deterministic sample avoids a
/// second full-size copy of large volumes.
pub(crate) fn volume_auto_color_range(
    values: &[f32],
    specials: &[u8],
    exact_range: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    const SAMPLE_LIMIT: usize = 500_000;
    let threshold = if values.len() <= SAMPLE_LIMIT {
        u128::from(u64::MAX)
    } else {
        (u128::from(u64::MAX) * SAMPLE_LIMIT as u128) / values.len() as u128
    };
    let mut sample = values
        .iter()
        .zip(specials)
        .enumerate()
        .filter_map(|(index, (&value, &special))| {
            let chosen = values.len() <= SAMPLE_LIMIT
                || u128::from((index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)) < threshold;
            (chosen && special == 0 && value.is_finite()).then_some(value)
        })
        .collect::<Vec<_>>();
    if sample.len() < 20 {
        return exact_range;
    }
    sample.sort_unstable_by(f32::total_cmp);
    let low = sample[sample.len() * 5 / 100] as f64;
    let high = sample[sample.len() * 95 / 100] as f64;
    (high > low).then_some((low, high)).or(exact_range)
}

/// Read every native voxel in the selected three-dimensional region.
/// The reader limits each hyperslab to one million values, so chunk along
/// the slowest varying selected axis to retain the source's row-major order.
pub(crate) fn load_exact_volume(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &VolumeSlice,
) -> Result<ExactVolumeData, String> {
    if shape.iter().any(|&length| length == 0) {
        return Err("Empty dataset".into());
    }
    let dimensions = std::array::from_fn::<_, 3, _>(|slot| {
        slice.axes[slot].map_or(1, |axis| {
            let [first, last] = slice.ranges[axis];
            last - first + 1
        })
    });
    let total = dimensions
        .iter()
        .try_fold(1usize, |n, &length| n.checked_mul(length))
        .ok_or("Volume dimensions overflow")?;
    if total > 64_000_000 {
        return Err(
            "More than 64 million voxels; crop the axis ranges for an exact 3D view".into(),
        );
    }
    let mut start = vec![0; shape.len()];
    let mut count = vec![1; shape.len()];
    if let Some(axis) = slice.hidden {
        start[axis] = slice.pinned;
    }
    for axis in slice.axes.iter().flatten().copied() {
        start[axis] = slice.ranges[axis][0];
        count[axis] = slice.ranges[axis][1] - slice.ranges[axis][0] + 1;
    }
    let slow_axis = slice
        .axes
        .iter()
        .flatten()
        .copied()
        .min()
        .ok_or("No visible axis")?;
    let plane = total / count[slow_axis];
    let chunk_size = (1_000_000 / plane).max(1);
    if plane > 1_000_000 {
        return Err("One volume plane exceeds the reader limit; crop the axis ranges".into());
    }
    let full_count = count[slow_axis];
    let full_start = start[slow_axis];
    let mut values = Vec::with_capacity(total);
    let mut specials = Vec::with_capacity(total);
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for offset in (0..full_count).step_by(chunk_size) {
        start[slow_axis] = full_start + offset;
        count[slow_axis] = chunk_size.min(full_count - offset);
        let region = file
            .region(path, &start, &count)
            .map_err(|err| err.to_string())?;
        for value in region.data {
            let special = SpecialValue::from_value(value, region.fill_value);
            specials.push(match special {
                None => 0,
                Some(SpecialValue::Fill) => 1,
                Some(SpecialValue::Nan) => 2,
                Some(SpecialValue::PositiveInf) => 3,
                Some(SpecialValue::NegativeInf) => 4,
            });
            if special.is_none() {
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
            values.push(value as f32);
        }
    }
    let mut stride = 1;
    let mut native_strides = vec![1; shape.len()];
    count[slow_axis] = full_count;
    for axis in (0..shape.len()).rev() {
        native_strides[axis] = stride;
        stride *= count[axis];
    }
    let strides =
        std::array::from_fn(|slot| slice.axes[slot].map_or(0, |axis| native_strides[axis]));
    let bounds = volume_visible_bounds(slice, shape);
    let range = (minimum <= maximum).then_some((minimum, maximum));
    let color_range = volume_auto_color_range(&values, &specials, range);
    Ok(ExactVolumeData {
        values,
        specials,
        dimensions,
        strides,
        bounds,
        color_range,
    })
}
