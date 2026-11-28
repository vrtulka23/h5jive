//! Path-based dataset slicing and arithmetic for plugin jobs.
//!
//! The UI describes an operation with paths and axis ranges; this module owns
//! staging, datatype selection, chunked HDF5 reads and writes, and publication.

use crate::{copy_hdf5_object, dataset_path_parts, Hdf5ObjectKind, StagedOutput, WritableFile};
use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Dataset, Hyperslab, SliceOrIndex,
};
use ndarray::{ArrayD, IxDyn};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DatasetOperation {
    #[default]
    Slice,
    Sum,
    Difference,
    Product,
    Ratio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisRange {
    pub start: usize,
    pub stop: usize,
    pub step: usize,
}

impl AxisRange {
    fn count(self, length: usize) -> Result<usize, String> {
        if self.step == 0 || self.start >= self.stop || self.stop > length {
            return Err(format!(
                "Expected 0 ≤ start < stop ≤ {length}, with step ≥ 1"
            ));
        }
        Ok((self.stop - self.start - 1) / self.step + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatasetOutputTarget {
    Existing(PathBuf),
    NewFile(PathBuf),
}

impl DatasetOutputTarget {
    fn path(&self) -> &Path {
        match self {
            Self::Existing(path) | Self::NewFile(path) => path,
        }
    }

    fn is_new_file(&self) -> bool {
        matches!(self, Self::NewFile(_))
    }
}

#[derive(Clone, Debug)]
pub struct DatasetOperationRequest {
    pub left_file: PathBuf,
    pub left_path: String,
    pub right: Option<(PathBuf, String)>,
    pub operation: DatasetOperation,
    pub ranges: Vec<AxisRange>,
    pub destination: DatasetOutputTarget,
    pub output_path: String,
    pub chunk_elements: usize,
}

/// Execute an operation in a staging file and publish only after it succeeds.
pub fn execute_dataset_operation(request: &DatasetOperationRequest) -> Result<String, String> {
    let (parent_path, name) =
        dataset_path_parts(&request.output_path).map_err(|e| e.to_string())?;
    let destination_path = request.destination.path();
    if request.destination.is_new_file() && destination_path.exists() {
        return Err("Output file already exists".into());
    }
    let staged_output =
        StagedOutput::new(destination_path, "operation", "hdf5").map_err(|e| e.to_string())?;
    let source_file = hdf5_metno::File::open(&request.left_file)
        .map_err(|e| format!("The source must be HDF5 or NetCDF-4: {e}"))?;
    let left = source_file
        .dataset(&request.left_path)
        .map_err(|e| e.to_string())?;
    let left_type = numeric_descriptor(&left)?;
    let right_file = request
        .right
        .as_ref()
        .map(|(path, _)| {
            hdf5_metno::File::open(path)
                .map_err(|e| format!("The second source must be HDF5 or NetCDF-4: {e}"))
        })
        .transpose()?;
    let right = request
        .right
        .as_ref()
        .zip(right_file.as_ref())
        .map(|((_, path), file)| file.dataset(path).map_err(|e| e.to_string()))
        .transpose()?;
    let right_type = right.as_ref().map(numeric_descriptor).transpose()?;
    let shape = left.shape();
    if shape.is_empty() || shape.iter().any(|&length| length == 0) {
        return Err("The source dataset must have nonempty dimensions".into());
    }
    if request.operation != DatasetOperation::Slice
        && right
            .as_ref()
            .is_none_or(|dataset| dataset.shape() != shape)
    {
        return Err("Arithmetic operations require equal dataset shapes".into());
    }
    let ranges = if request.operation == DatasetOperation::Slice {
        if request.ranges.len() != shape.len() {
            return Err("Slice dimensions changed".into());
        }
        request.ranges.clone()
    } else {
        shape
            .iter()
            .map(|&stop| AxisRange {
                start: 0,
                stop,
                step: 1,
            })
            .collect()
    };
    let output_shape = ranges
        .iter()
        .zip(&shape)
        .map(|(&range, &length)| range.count(length))
        .collect::<Result<Vec<_>, _>>()?;
    let left_fill = fill_value(&left);
    let right_fill = right.as_ref().and_then(fill_value);
    let result_type = output_type(
        &left_type,
        right_type.as_ref(),
        request.operation,
        left_fill.is_some() || right_fill.is_some(),
    );
    let stage = hdf5_metno::File::create(staged_output.path()).map_err(|e| e.to_string())?;
    let group = if parent_path == "/" {
        stage.group("/")
    } else {
        stage.create_group(parent_path)
    }
    .map_err(|e| e.to_string())?;
    let target = match result_type {
        OutputType::Source => group
            .new_dataset_builder()
            .empty_as(&left.dtype().map_err(|e| e.to_string())?)
            .shape(output_shape.clone())
            .create(name),
        OutputType::Integer => group
            .new_dataset::<i64>()
            .shape(output_shape.clone())
            .create(name),
        OutputType::F32 => group
            .new_dataset::<f32>()
            .shape(output_shape.clone())
            .create(name),
        OutputType::F64 => group
            .new_dataset::<f64>()
            .shape(output_shape.clone())
            .create(name),
    }
    .map_err(|e| e.to_string())?;
    tiles(&output_shape, request.chunk_elements, |offset, count| {
        let input = selection(&ranges, offset, count, true);
        let output = selection(&ranges, offset, count, false);
        if request.operation == DatasetOperation::Slice {
            copy_slice(&left, &target, &left_type, input, output)
        } else {
            calculate(
                &left,
                right.as_ref().unwrap(),
                &target,
                &left_type,
                right_type.as_ref().unwrap(),
                result_type,
                request.operation,
                left_fill,
                right_fill,
                input,
                output,
                count,
            )
        }
    })?;
    stage.flush().map_err(|e| e.to_string())?;
    drop(target);
    drop(group);
    drop(stage);
    drop(right);
    drop(right_file);
    drop(left);
    drop(source_file);
    if request.destination.is_new_file() {
        staged_output
            .publish_new(destination_path)
            .map_err(|e| e.to_string())?;
    } else {
        let destination =
            WritableFile::open(destination_path).map_err(|error| error.to_string())?;
        if destination.link_exists(&request.output_path) {
            return Err("Output dataset already exists".into());
        }
        let stage = hdf5_metno::File::open(staged_output.path()).map_err(|e| e.to_string())?;
        let parent = if parent_path == "/" {
            destination.group("/")
        } else if destination.link_exists(parent_path) {
            destination.group(parent_path)
        } else {
            destination.create_group(parent_path)
        }
        .map_err(|e| e.to_string())?;
        copy_hdf5_object(
            &stage,
            &request.output_path,
            &parent,
            name,
            Hdf5ObjectKind::Dataset,
            None,
        )
        .map_err(|e| e.to_string())?;
        destination.flush().map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "Saved {} in {}",
        request.output_path,
        destination_path.display()
    ))
}

fn numeric_descriptor(dataset: &Dataset) -> Result<TypeDescriptor, String> {
    let descriptor = dataset
        .dtype()
        .and_then(|dtype| dtype.to_descriptor())
        .map_err(|e| e.to_string())?;
    if matches!(
        descriptor,
        TypeDescriptor::Integer(_) | TypeDescriptor::Unsigned(_) | TypeDescriptor::Float(_)
    ) {
        Ok(descriptor)
    } else {
        Err(format!("Unsupported numeric datatype: {descriptor}"))
    }
}

#[derive(Clone, Copy)]
enum OutputType {
    Source,
    Integer,
    F32,
    F64,
}

fn output_type(
    left: &TypeDescriptor,
    right: Option<&TypeDescriptor>,
    operation: DatasetOperation,
    has_fill: bool,
) -> OutputType {
    if operation == DatasetOperation::Slice {
        return OutputType::Source;
    }
    let right = right.expect("Arithmetic requires a second dataset");
    if *left == TypeDescriptor::Float(FloatSize::U4)
        && *right == TypeDescriptor::Float(FloatSize::U4)
    {
        return OutputType::F32;
    }
    if matches!(
        operation,
        DatasetOperation::Sum | DatasetOperation::Difference | DatasetOperation::Product
    ) && !has_fill
        && matches!(
            left,
            TypeDescriptor::Integer(_) | TypeDescriptor::Unsigned(_)
        )
        && matches!(
            right,
            TypeDescriptor::Integer(_) | TypeDescriptor::Unsigned(_)
        )
    {
        return OutputType::Integer;
    }
    OutputType::F64
}

fn fill_value(dataset: &Dataset) -> Option<f64> {
    dataset
        .attr("_FillValue")
        .ok()
        .and_then(|attr| attr.read_scalar::<f64>().ok())
}

fn selection(ranges: &[AxisRange], offset: &[usize], count: &[usize], input: bool) -> Hyperslab {
    let dimensions = ranges
        .iter()
        .enumerate()
        .map(|(axis, range)| SliceOrIndex::SliceCount {
            start: if input {
                range.start + offset[axis] * range.step
            } else {
                offset[axis]
            },
            step: if input { range.step } else { 1 },
            count: count[axis],
            block: 1,
        })
        .collect::<Vec<_>>();
    Hyperslab::new(dimensions)
}

fn tiles(
    shape: &[usize],
    max_elements: usize,
    mut visit: impl FnMut(&[usize], &[usize]) -> Result<(), String>,
) -> Result<(), String> {
    let mut capacity = max_elements.max(1);
    let mut tile = vec![1; shape.len()];
    for axis in (0..shape.len()).rev() {
        tile[axis] = shape[axis].min(capacity.max(1));
        capacity = (capacity / tile[axis]).max(1);
    }
    let mut offset = vec![0; shape.len()];
    loop {
        let count = offset
            .iter()
            .enumerate()
            .map(|(i, &start)| tile[i].min(shape[i] - start))
            .collect::<Vec<_>>();
        visit(&offset, &count)?;
        let mut axis = shape.len();
        loop {
            if axis == 0 {
                return Ok(());
            }
            axis -= 1;
            offset[axis] += tile[axis];
            if offset[axis] < shape[axis] {
                break;
            }
            offset[axis] = 0;
        }
    }
}

fn read_integer(
    dataset: &Dataset,
    descriptor: &TypeDescriptor,
    where_: Hyperslab,
) -> Result<Vec<i128>, String> {
    macro_rules! read_as {
        ($ty:ty) => {
            dataset
                .read_slice::<$ty, _, IxDyn>(where_)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|&value| value as i128)
                .collect()
        };
    }
    Ok(match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => read_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => read_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => read_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => read_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => read_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => read_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => read_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => read_as!(u64),
        _ => return Err("Expected an integer dataset".into()),
    })
}

fn copy_slice(
    source: &Dataset,
    target: &Dataset,
    descriptor: &TypeDescriptor,
    input: Hyperslab,
    output: Hyperslab,
) -> Result<(), String> {
    macro_rules! copy_as {
        ($ty:ty) => {{
            let values = source
                .read_slice::<$ty, _, IxDyn>(input)
                .map_err(|e| e.to_string())?;
            target
                .write_slice(values.view(), output)
                .map_err(|e| e.to_string())
        }};
    }
    match descriptor {
        TypeDescriptor::Integer(IntSize::U1) => copy_as!(i8),
        TypeDescriptor::Integer(IntSize::U2) => copy_as!(i16),
        TypeDescriptor::Integer(IntSize::U4) => copy_as!(i32),
        TypeDescriptor::Integer(IntSize::U8) => copy_as!(i64),
        TypeDescriptor::Unsigned(IntSize::U1) => copy_as!(u8),
        TypeDescriptor::Unsigned(IntSize::U2) => copy_as!(u16),
        TypeDescriptor::Unsigned(IntSize::U4) => copy_as!(u32),
        TypeDescriptor::Unsigned(IntSize::U8) => copy_as!(u64),
        TypeDescriptor::Float(FloatSize::U4) => copy_as!(f32),
        TypeDescriptor::Float(FloatSize::U8) => copy_as!(f64),
        _ => Err("Unsupported slicing datatype".into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate(
    left: &Dataset,
    right: &Dataset,
    target: &Dataset,
    left_type: &TypeDescriptor,
    right_type: &TypeDescriptor,
    output_type: OutputType,
    operation: DatasetOperation,
    left_fill: Option<f64>,
    right_fill: Option<f64>,
    input: Hyperslab,
    output: Hyperslab,
    count: &[usize],
) -> Result<(), String> {
    if matches!(output_type, OutputType::Integer) {
        let a = read_integer(left, left_type, input.clone())?;
        let b = read_integer(right, right_type, input)?;
        let values = a
            .into_iter()
            .zip(b)
            .map(|(a, b)| {
                let result = match operation {
                    DatasetOperation::Sum => a.checked_add(b),
                    DatasetOperation::Difference => a.checked_sub(b),
                    DatasetOperation::Product => a.checked_mul(b),
                    DatasetOperation::Slice | DatasetOperation::Ratio => {
                        unreachable!("integer arithmetic output")
                    }
                };
                result
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or_else(|| "Integer result exceeds the i64 range".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let array = ArrayD::from_shape_vec(IxDyn(count), values).map_err(|e| e.to_string())?;
        return target
            .write_slice(array.view(), output)
            .map_err(|e| e.to_string());
    }
    if matches!(output_type, OutputType::F32) {
        let a = left
            .read_slice::<f32, _, IxDyn>(input.clone())
            .map_err(|e| e.to_string())?;
        let b = right
            .read_slice::<f32, _, IxDyn>(input)
            .map_err(|e| e.to_string())?;
        let values = a
            .iter()
            .zip(b.iter())
            .map(|(&a, &b)| {
                if left_fill == Some(a as f64) || right_fill == Some(b as f64) {
                    f32::NAN
                } else {
                    match operation {
                        DatasetOperation::Sum => a + b,
                        DatasetOperation::Difference => a - b,
                        DatasetOperation::Product => a * b,
                        DatasetOperation::Ratio if b == 0.0 => f32::NAN,
                        DatasetOperation::Ratio => a / b,
                        DatasetOperation::Slice => unreachable!("slice uses copy_slice"),
                    }
                }
            })
            .collect::<Vec<_>>();
        let array = ArrayD::from_shape_vec(IxDyn(count), values).map_err(|e| e.to_string())?;
        return target
            .write_slice(array.view(), output)
            .map_err(|e| e.to_string());
    }
    let a = left
        .read_slice::<f64, _, IxDyn>(input.clone())
        .map_err(|e| e.to_string())?;
    let b = right
        .read_slice::<f64, _, IxDyn>(input)
        .map_err(|e| e.to_string())?;
    let values = a
        .iter()
        .zip(b.iter())
        .map(|(&a, &b)| {
            if left_fill == Some(a) || right_fill == Some(b) {
                f64::NAN
            } else {
                match operation {
                    DatasetOperation::Sum => a + b,
                    DatasetOperation::Difference => a - b,
                    DatasetOperation::Product => a * b,
                    DatasetOperation::Ratio if b == 0.0 => f64::NAN,
                    DatasetOperation::Ratio => a / b,
                    DatasetOperation::Slice => unreachable!("slice uses copy_slice"),
                }
            }
        })
        .collect::<Vec<_>>();
    let array = ArrayD::from_shape_vec(IxDyn(count), values).map_err(|e| e.to_string())?;
    target
        .write_slice(array.view(), output)
        .map_err(|e| e.to_string())
}
