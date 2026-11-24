use super::*;
use h5jive_data::{copy_hdf5_object, dataset_path_parts, Hdf5ObjectKind, StagedOutput};
use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Dataset, Hyperslab, SliceOrIndex,
};
use ndarray::{ArrayD, IxDyn};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Operation {
    #[default]
    Slice,
    Sum,
    Difference,
    Product,
    Ratio,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Destination {
    #[default]
    A,
    B,
    NewFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AxisRange {
    start: usize,
    stop: usize,
    step: usize,
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

#[derive(Clone)]
struct JobSpec {
    left_file: PathBuf,
    left_path: String,
    right: Option<(PathBuf, String)>,
    operation: Operation,
    ranges: Vec<AxisRange>,
    destination: Destination,
    destination_file: PathBuf,
    output_path: String,
    chunk_elements: usize,
}

pub(super) struct State {
    choices: Vec<DatasetChoice>,
    left: usize,
    right: usize,
    left_search: String,
    right_search: String,
    restore_left: Option<(usize, String)>,
    restore_right: Option<(usize, String)>,
    operation: Operation,
    destination: Destination,
    ranges: Vec<[String; 3]>,
    output_path: String,
    pub(super) new_file: String,
    job: BackgroundJob<Destination, String>,
    status: Option<Result<String, String>>,
    reload: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            choices: Vec::new(),
            left: 0,
            right: 0,
            left_search: String::new(),
            right_search: String::new(),
            restore_left: None,
            restore_right: None,
            operation: Operation::Slice,
            destination: Destination::A,
            ranges: Vec::new(),
            output_path: "/result".into(),
            new_file: String::new(),
            job: BackgroundJob::default(),
            status: None,
            reload: false,
        }
    }
}

impl State {
    pub fn invalidate(&mut self) {
        if !self.choices.is_empty() {
            self.restore_left = self
                .choices
                .get(self.left)
                .map(|choice| (choice.file, choice.path.clone()));
            self.restore_right = self
                .choices
                .get(self.right)
                .map(|choice| (choice.file, choice.path.clone()));
        }
        self.choices.clear();
    }
    pub fn is_busy(&mut self) -> bool {
        let mut updated = false;
        if let Some((destination, result)) = self.job.poll("Dataset operation worker stopped") {
            self.reload = destination != Destination::NewFile;
            self.status = Some(result);
            updated = true;
        }
        self.job.is_busy() || updated
    }
    pub fn take_reload(&mut self) -> bool {
        std::mem::take(&mut self.reload)
    }
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
    operation: Operation,
    has_fill: bool,
) -> OutputType {
    if operation == Operation::Slice {
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
        Operation::Sum | Operation::Difference | Operation::Product
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

fn calculate(
    left: &Dataset,
    right: &Dataset,
    target: &Dataset,
    left_type: &TypeDescriptor,
    right_type: &TypeDescriptor,
    output_type: OutputType,
    operation: Operation,
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
                    Operation::Sum => a.checked_add(b),
                    Operation::Difference => a.checked_sub(b),
                    Operation::Product => a.checked_mul(b),
                    Operation::Slice | Operation::Ratio => {
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
                        Operation::Sum => a + b,
                        Operation::Difference => a - b,
                        Operation::Product => a * b,
                        Operation::Ratio if b == 0.0 => f32::NAN,
                        Operation::Ratio => a / b,
                        Operation::Slice => unreachable!("slice uses copy_slice"),
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
                    Operation::Sum => a + b,
                    Operation::Difference => a - b,
                    Operation::Product => a * b,
                    Operation::Ratio if b == 0.0 => f64::NAN,
                    Operation::Ratio => a / b,
                    Operation::Slice => unreachable!("slice uses copy_slice"),
                }
            }
        })
        .collect::<Vec<_>>();
    let array = ArrayD::from_shape_vec(IxDyn(count), values).map_err(|e| e.to_string())?;
    target
        .write_slice(array.view(), output)
        .map_err(|e| e.to_string())
}

fn run(spec: JobSpec) -> Result<String, String> {
    let (parent_path, name) = dataset_path_parts(&spec.output_path).map_err(|e| e.to_string())?;
    if spec.destination == Destination::NewFile && spec.destination_file.exists() {
        return Err("Output file already exists".into());
    }
    let staged_output = StagedOutput::new(&spec.destination_file, "operation", "hdf5")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<String, String> {
        let source_file = hdf5_metno::File::open(&spec.left_file)
            .map_err(|e| format!("The source must be HDF5 or NetCDF-4: {e}"))?;
        let left = source_file
            .dataset(&spec.left_path)
            .map_err(|e| e.to_string())?;
        let left_type = numeric_descriptor(&left)?;
        let right_file = spec
            .right
            .as_ref()
            .map(|(path, _)| {
                hdf5_metno::File::open(path)
                    .map_err(|e| format!("The second source must be HDF5 or NetCDF-4: {e}"))
            })
            .transpose()?;
        let right = spec
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
        if spec.operation != Operation::Slice
            && right
                .as_ref()
                .is_none_or(|dataset| dataset.shape() != shape)
        {
            return Err("Arithmetic operations require equal dataset shapes".into());
        }
        let ranges = if spec.operation == Operation::Slice {
            if spec.ranges.len() != shape.len() {
                return Err("Slice dimensions changed".into());
            }
            spec.ranges.clone()
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
            spec.operation,
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
        tiles(&output_shape, spec.chunk_elements, |offset, count| {
            let input = selection(&ranges, offset, count, true);
            let output = selection(&ranges, offset, count, false);
            if spec.operation == Operation::Slice {
                copy_slice(&left, &target, &left_type, input, output)
            } else {
                calculate(
                    &left,
                    right.as_ref().unwrap(),
                    &target,
                    &left_type,
                    right_type.as_ref().unwrap(),
                    result_type,
                    spec.operation,
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
        if spec.destination == Destination::NewFile {
            staged_output
                .publish_new(&spec.destination_file)
                .map_err(|e| e.to_string())?;
        } else {
            let destination =
                WritableFile::open(&spec.destination_file).map_err(|error| error.to_string())?;
            if destination.link_exists(&spec.output_path) {
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
                &spec.output_path,
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
            spec.output_path,
            spec.destination_file.display()
        ))
    })();
    result
}

fn dataset_picker(
    ui: &Ui,
    title: &str,
    id: &str,
    choices: &[DatasetChoice],
    selected: &mut usize,
    search: &mut String,
    shape: Option<&[usize]>,
    tutorial_tips: bool,
) -> bool {
    let visible: Vec<_> = choices
        .iter()
        .filter(|choice| shape.is_none_or(|required| choice.shape == required))
        .collect();
    let picked = dataset_choice_picker(
        ui,
        &format!("manipulation_{id}"),
        title,
        &visible,
        choices.get(*selected),
        search,
        tutorial_tips,
    );
    if let Some(index) = picked.and_then(|choice| choices.iter().position(|item| *item == choice)) {
        let changed = *selected != index;
        *selected = index;
        changed
    } else {
        false
    }
}

pub(super) fn draw_icon(draw: &DrawListMut<'_>, min: [f32; 2], max: [f32; 2], color: [f32; 4]) {
    let x = (min[0] + max[0]) * 0.5;
    let y = (min[1] + max[1]) * 0.5;
    draw.add_rect([x - 12.0, y - 11.0], [x + 12.0, y + 11.0], color)
        .thickness(1.6)
        .build();
    draw.add_line([x - 5.0, y], [x + 5.0, y], color)
        .thickness(1.6)
        .build();
    draw.add_line([x, y - 5.0], [x, y + 5.0], color)
        .thickness(1.6)
        .build();
}

#[derive(Clone, Copy)]
pub(super) enum SegmentIcon {
    Slice,
    Add,
    Subtract,
    Multiply,
    Divide,
    File,
    NewFile,
    Native,
    TimeSeries,
    Mosaic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SegmentLayout {
    Horizontal,
    Vertical,
}

fn draw_segment_icon(ui: &Ui, icon: SegmentIcon, color: [f32; 4]) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let x = min[0] + 15.0;
    let y = (min[1] + max[1]) * 0.5;
    let draw = ui.get_window_draw_list();
    match icon {
        SegmentIcon::Slice => {
            for edge in [x - 6.0, x + 6.0] {
                draw.add_line([edge, y - 7.0], [edge, y + 7.0], color)
                    .thickness(1.6)
                    .build();
            }
            draw.add_line([x - 2.5, y], [x + 2.5, y], color)
                .thickness(2.0)
                .build();
        }
        SegmentIcon::Add | SegmentIcon::Subtract | SegmentIcon::Multiply | SegmentIcon::Divide => {
            if !matches!(icon, SegmentIcon::Multiply) {
                draw.add_line([x - 6.0, y], [x + 6.0, y], color)
                    .thickness(1.8)
                    .build();
            }
            if matches!(icon, SegmentIcon::Add) {
                draw.add_line([x, y - 6.0], [x, y + 6.0], color)
                    .thickness(1.8)
                    .build();
            }
            if matches!(icon, SegmentIcon::Multiply) {
                draw.add_line([x - 4.5, y - 4.5], [x + 4.5, y + 4.5], color)
                    .thickness(1.8)
                    .build();
                draw.add_line([x - 4.5, y + 4.5], [x + 4.5, y - 4.5], color)
                    .thickness(1.8)
                    .build();
            }
            if matches!(icon, SegmentIcon::Divide) {
                for dot_y in [y - 5.5, y + 5.5] {
                    draw.add_circle([x, dot_y], 1.3, color).filled(true).build();
                }
            }
        }
        SegmentIcon::File | SegmentIcon::NewFile => {
            draw.add_rect([x - 5.0, y - 7.0], [x + 5.0, y + 7.0], color)
                .thickness(1.4)
                .build();
            if matches!(icon, SegmentIcon::NewFile) {
                draw.add_line([x - 2.5, y], [x + 2.5, y], color)
                    .thickness(1.4)
                    .build();
                draw.add_line([x, y - 2.5], [x, y + 2.5], color)
                    .thickness(1.4)
                    .build();
            } else {
                draw.add_line([x - 2.5, y - 2.0], [x + 2.5, y - 2.0], color)
                    .thickness(1.2)
                    .build();
            }
        }
        SegmentIcon::Native => {
            for shift in [-2.5, 2.5] {
                draw.add_rect(
                    [x - 6.0 + shift, y - 6.0 + shift],
                    [x + 3.0 + shift, y + 5.0 + shift],
                    color,
                )
                .thickness(1.4)
                .build();
            }
        }
        SegmentIcon::TimeSeries => {
            for shift in [-4.0, 0.0, 4.0] {
                draw.add_circle([x + shift, y], 1.5, color)
                    .filled(true)
                    .build();
            }
            draw.add_line([x - 6.0, y + 5.0], [x + 6.0, y + 5.0], color)
                .thickness(1.4)
                .build();
        }
        SegmentIcon::Mosaic => {
            for row in 0..2 {
                for column in 0..2 {
                    let left = x - 6.0 + column as f32 * 7.0;
                    let top = y - 6.0 + row as f32 * 7.0;
                    draw.add_rect([left, top], [left + 5.0, top + 5.0], color)
                        .thickness(1.4)
                        .build();
                }
            }
        }
    }
}

pub(super) fn segmented_control<T: Copy + PartialEq>(
    ui: &Ui,
    id: &str,
    selected: &mut T,
    options: &[(T, &str, SegmentIcon, &str)],
    layout: SegmentLayout,
    max_width: Option<f32>,
    tutorial_tips: bool,
    accent: [f32; 4],
    border: [f32; 4],
) -> bool {
    let _id = ui.push_id(id);
    let base = ui.style_color(StyleColor::Button);
    let mut changed = false;
    let mut group_min = [0.0; 2];
    let mut group_max = [0.0; 2];
    let _rounding = ui.push_style_var(StyleVar::FrameRounding(0.0));
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
    let vertical_width = ui.content_region_avail()[0]
        .min(max_width.unwrap_or(f32::INFINITY))
        .max(1.0);
    for (index, &(value, label, icon, tip)) in options.iter().enumerate() {
        if index > 0 && layout == SegmentLayout::Horizontal {
            ui.same_line_with_spacing(0.0, 0.0);
        }
        let active = *selected == value;
        let background = if active {
            mix_color(base, accent, 0.65)
        } else {
            base
        };
        let _button = ui.push_style_color(StyleColor::Button, background);
        let _hover = ui.push_style_color(
            StyleColor::ButtonHovered,
            mix_color(background, accent, 0.25),
        );
        let _pressed = ui.push_style_color(StyleColor::ButtonActive, accent);
        let width = if layout == SegmentLayout::Vertical {
            vertical_width
        } else {
            ui.calc_text_size(label)[0] + 44.0
        };
        if ui.button_with_size(format!("    {label}##{index}"), [width, 30.0]) && !active {
            *selected = value;
            changed = true;
        }
        let min = ui.item_rect_min();
        let max = ui.item_rect_max();
        if index == 0 {
            group_min = min;
        } else {
            let draw = ui.get_window_draw_list();
            if layout == SegmentLayout::Vertical {
                draw.add_line([min[0] + 3.0, min[1]], [max[0] - 3.0, min[1]], border)
                    .build();
            } else {
                draw.add_line([min[0], min[1] + 3.0], [min[0], max[1] - 3.0], border)
                    .build();
            }
        }
        group_max = max;
        draw_segment_icon(ui, icon, ui.style_color(StyleColor::Text));
        item_tip(ui, tutorial_tips, tip, tip);
    }
    ui.get_window_draw_list()
        .add_rect(group_min, group_max, border)
        .rounding(4.0)
        .build();
    changed
}

pub(super) fn draw(lab: &mut PluginLab, ui: &Ui, context: PluginContext<'_>) -> Option<Selection> {
    let state = &mut lab.dataset_manipulation;
    if state.choices.is_empty() {
        state.choices = numeric_datasets(context.files, 1);
        if let Some((file, path)) = state.restore_left.take() {
            if let Some(index) = state
                .choices
                .iter()
                .position(|choice| choice.file == file && choice.path == path)
            {
                state.left = index;
            } else {
                state.left = 0;
                state.ranges.clear();
            }
        }
        if let Some((file, path)) = state.restore_right.take() {
            state.right = state
                .choices
                .iter()
                .position(|choice| choice.file == file && choice.path == path)
                .unwrap_or(0);
        }
    }
    if state.choices.is_empty() {
        ui.text_wrapped("Load an HDF5 or NetCDF-4 file with a numeric dataset.");
        return None;
    }
    state.left = state.left.min(state.choices.len() - 1);
    state.right = state.right.min(state.choices.len() - 1);
    if let Some(_table) = ui.begin_table("manipulation_inputs", 2) {
        ui.table_setup_column_fixed_width("Operation", TableColumnFlags::NONE, 160.0);
        ui.table_setup_column_stretch_weight("Datasets", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        ui.text("Operation");
        if segmented_control(
            ui,
            "manipulation_operation",
            &mut state.operation,
            &[
                (
                    Operation::Slice,
                    "Slice",
                    SegmentIcon::Slice,
                    "Select a range of values from the source dataset.",
                ),
                (
                    Operation::Sum,
                    "Add",
                    SegmentIcon::Add,
                    "Create Left + Right from two matching datasets.",
                ),
                (
                    Operation::Difference,
                    "Subtract",
                    SegmentIcon::Subtract,
                    "Create Left - Right from two matching datasets.",
                ),
                (
                    Operation::Product,
                    "Multiply",
                    SegmentIcon::Multiply,
                    "Create Left * Right from two matching datasets.",
                ),
                (
                    Operation::Ratio,
                    "Divide",
                    SegmentIcon::Divide,
                    "Create Left / Right from two matching datasets.",
                ),
            ],
            SegmentLayout::Vertical,
            None,
            context.settings.tutorial_tips,
            context.settings.palette.accent,
            context.settings.palette.border,
        ) {
            state.status = None;
        }
        ui.table_next_column();
        if dataset_picker(
            ui,
            "Source",
            "left",
            &state.choices,
            &mut state.left,
            &mut state.left_search,
            None,
            context.settings.tutorial_tips,
        ) {
            state.ranges.clear();
            state.status = None;
            let shape = &state.choices[state.left].shape;
            if state.choices[state.right].shape != *shape {
                state.right = state
                    .choices
                    .iter()
                    .position(|choice| choice.shape == *shape)
                    .unwrap_or(0);
            }
        }
        let left = &state.choices[state.left];
        if state.operation == Operation::Slice {
            if state.ranges.len() != left.shape.len() {
                state.ranges = left
                    .shape
                    .iter()
                    .map(|&length| ["0".into(), length.to_string(), "1".into()])
                    .collect();
            }
            ui.text("Slice ranges (start inclusive, stop exclusive)");
            item_tip(ui, context.settings.tutorial_tips, "", "Set a start, stop, and step for each dimension. The result keeps every selected index in each dimension.");
            if let Some(_ranges) = ui.begin_table("manipulation_slice_ranges", 4) {
                let axis_width = (ui.content_region_avail()[0] * 0.22).clamp(76.0, 120.0);
                ui.table_setup_column_fixed_width("Axis", TableColumnFlags::NONE, axis_width);
                for label in ["Start", "Stop", "Step"] {
                    ui.table_setup_column_stretch_weight(label, TableColumnFlags::NONE, 1.0);
                }
                ui.table_headers_row();
                for (axis, fields) in state.ranges.iter_mut().enumerate() {
                    ui.table_next_row();
                    ui.table_next_column();
                    ui.text(format!("{axis} (len {})", left.shape[axis]));
                    for (field, name) in fields.iter_mut().zip(["start", "stop", "step"]) {
                        ui.table_next_column();
                        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
                        ui.input_text(format!("##manip_{name}_{axis}"), field)
                            .build();
                    }
                }
            }
        } else {
            dataset_picker(
                ui,
                "Second dataset",
                "right",
                &state.choices,
                &mut state.right,
                &mut state.right_search,
                Some(&left.shape),
                context.settings.tutorial_tips,
            );
            if state.choices[state.right].shape != left.shape {
                ui.text_disabled("Arithmetic requires datasets with identical shapes.");
            }
        }
    }
    let left = &state.choices[state.left];
    ui.separator();
    if let Some(_table) = ui.begin_table("manipulation_output", 2) {
        ui.table_setup_column_fixed_width("Destination", TableColumnFlags::NONE, 160.0);
        ui.table_setup_column_stretch_weight("Output", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        ui.text("Save into");
        segmented_control(
            ui,
            "manipulation_destination",
            &mut state.destination,
            &[
                (
                    Destination::A,
                    "File A",
                    SegmentIcon::File,
                    "Save a new dataset in file A.",
                ),
                (
                    Destination::B,
                    "File B",
                    SegmentIcon::File,
                    "Save a new dataset in file B.",
                ),
                (
                    Destination::NewFile,
                    "New file",
                    SegmentIcon::NewFile,
                    "Save to a new HDF5 file.",
                ),
            ],
            SegmentLayout::Vertical,
            None,
            context.settings.tutorial_tips,
            context.settings.palette.accent,
            context.settings.palette.border,
        );
        ui.table_next_column();
        ui.text("Destination");
        if state.destination == Destination::NewFile {
            ui.text("New HDF5 file");
            if file_path_field(
                ui,
                "manipulation_new_file",
                &mut state.new_file,
                ui.content_region_avail()[0],
            ) {
                *context.picker_request =
                    Some((FilePickerTarget::ManipulationOutput, state.new_file.clone()));
            }
            item_tip(
                ui,
                context.settings.tutorial_tips,
                "",
                "Enter a new .hdf5 or .h5 filename; existing files are protected.",
            );
        }
        ui.input_text("New dataset path", &mut state.output_path)
            .build();
        item_tip(
            ui,
            context.settings.tutorial_tips,
            "",
            "Use an absolute path such as /processed/slice. Existing datasets are never replaced.",
        );
        ui.text_disabled("Source attributes are not copied to the result.");
        if state.job.is_busy() {
            ui.text("Writing dataset...");
        }
        if let Some(status) = &state.status {
            match status {
                Ok(message) => ui.text_wrapped(message),
                Err(error) => ui.text_wrapped(format!("Could not save dataset: {error}")),
            }
        }
        ui.spacing();
        let create_clicked = {
            let _disabled = ui.begin_disabled_with_cond(state.job.is_busy());
            let palette = &context.settings.palette;
            let _button = ui.push_style_color(StyleColor::Button, palette.accent);
            let _hover = ui.push_style_color(
                StyleColor::ButtonHovered,
                mix_color(palette.accent, palette.text, 0.18),
            );
            let _active = ui.push_style_color(
                StyleColor::ButtonActive,
                mix_color(palette.accent, palette.panel, 0.2),
            );
            ui.button("Create dataset")
        };
        if create_clicked {
            let ranges = if state.operation == Operation::Slice {
                state
                    .ranges
                    .iter()
                    .map(|fields| {
                        let parse = |text: &str| {
                            text.trim()
                                .parse::<usize>()
                                .map_err(|_| format!("Invalid slice index: {text}"))
                        };
                        Ok(AxisRange {
                            start: parse(&fields[0])?,
                            stop: parse(&fields[1])?,
                            step: parse(&fields[2])?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
            } else {
                Ok(Vec::new())
            };
            let destination_file = match state.destination {
                Destination::A => context.file_paths.first().cloned(),
                Destination::B => context.file_paths.get(1).cloned(),
                Destination::NewFile => Some(PathBuf::from(state.new_file.trim())),
            };
            let spec = ranges.and_then(|ranges| {
                let destination_file = destination_file.ok_or("Load file B to save there")?;
                if state.destination == Destination::NewFile && state.new_file.trim().is_empty() {
                    return Err("Enter a new output filename".into());
                }
                dataset_path_parts(state.output_path.trim()).map_err(|e| e.to_string())?;
                let right = if state.operation == Operation::Slice {
                    None
                } else {
                    let choice = &state.choices[state.right];
                    if choice.shape != left.shape {
                        return Err("Both datasets must have identical shapes".into());
                    }
                    Some((context.file_paths[choice.file].clone(), choice.path.clone()))
                };
                Ok(JobSpec {
                    left_file: context.file_paths[left.file].clone(),
                    left_path: left.path.clone(),
                    right,
                    operation: state.operation,
                    ranges,
                    destination: state.destination,
                    destination_file,
                    output_path: state.output_path.trim().to_owned(),
                    chunk_elements: context.settings.dataset_manipulation_chunk_elements,
                })
            });
            match spec {
                Ok(spec) => {
                    let release = match spec.destination {
                        Destination::A | Destination::B => {
                            let slot = if spec.destination == Destination::A {
                                0
                            } else {
                                1
                            };
                            if !context.write_allowed[slot] {
                                Err("Finish file loading or synchronization and apply or discard pending inspector edits before writing to A/B".into())
                            } else {
                                context.files[slot]
                                    .release_for_edit()
                                    .map_err(|error| error.to_string())
                            }
                        }
                        Destination::NewFile => Ok(()),
                    };
                    match release {
                        Ok(()) => {
                            let spec_destination = spec.destination;
                            state.job.start(spec_destination, move |_| run(spec));
                            state.status = None;
                        }
                        Err(error) => state.status = Some(Err(error)),
                    }
                }
                Err(error) => state.status = Some(Err(error)),
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};
    use std::{
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "h5jive-manipulation-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let a = directory.join("a.hdf5");
        let b = directory.join("b.hdf5");
        for (path, offset) in [(&a, 0_i16), (&b, 2_i16)] {
            let file = hdf5_metno::File::create(path).unwrap();
            let values = (0..12).map(|value| value + offset).collect::<Vec<_>>();
            file.new_dataset::<i16>()
                .shape([3, 4])
                .create("x")
                .unwrap()
                .write_raw(&values)
                .unwrap();
            file.new_dataset::<f32>()
                .shape([3, 4])
                .create("float32")
                .unwrap()
                .write_raw(&vec![offset as f32 + 1.0; 12])
                .unwrap();
            file.new_dataset::<f64>()
                .shape([3, 4])
                .create("float64")
                .unwrap()
                .write_raw(&vec![offset as f64 + 1.0; 12])
                .unwrap();
        }
        (directory, a, b)
    }

    fn spec(a: &Path, b: &Path, output: &Path, operation: Operation) -> JobSpec {
        JobSpec {
            left_file: a.to_owned(),
            left_path: "/x".into(),
            right: (operation != Operation::Slice).then(|| (b.to_owned(), "/x".into())),
            operation,
            ranges: vec![
                AxisRange {
                    start: 1,
                    stop: 3,
                    step: 1,
                },
                AxisRange {
                    start: 0,
                    stop: 4,
                    step: 2,
                },
            ],
            destination: Destination::NewFile,
            destination_file: output.to_owned(),
            output_path: "/derived/result".into(),
            chunk_elements: 3,
        }
    }

    #[test]
    fn slice_preserves_integer_type_and_selected_indexes() {
        let (directory, a, b) = fixture();
        let output = directory.join("slice.hdf5");
        run(spec(&a, &b, &output, Operation::Slice)).unwrap();
        let file = hdf5_metno::File::open(&output).unwrap();
        let dataset = file.dataset("/derived/result").unwrap();
        assert_eq!(dataset.shape(), vec![2, 2]);
        assert_eq!(
            dataset.dtype().unwrap().to_descriptor().unwrap(),
            TypeDescriptor::Integer(IntSize::U2)
        );
        assert_eq!(dataset.read_raw::<i16>().unwrap(), vec![4, 6, 8, 10]);
        drop(dataset);
        drop(file);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn integer_difference_stays_integer_and_ratio_becomes_float() {
        let (directory, a, b) = fixture();
        let difference = directory.join("difference.hdf5");
        run(spec(&a, &b, &difference, Operation::Difference)).unwrap();
        let file = hdf5_metno::File::open(&difference).unwrap();
        let dataset = file.dataset("/derived/result").unwrap();
        assert_eq!(
            dataset.dtype().unwrap().to_descriptor().unwrap(),
            TypeDescriptor::Integer(IntSize::U8)
        );
        assert_eq!(dataset.read_raw::<i64>().unwrap(), vec![-2; 12]);
        drop(dataset);
        drop(file);
        let ratio = directory.join("ratio.hdf5");
        run(spec(&a, &b, &ratio, Operation::Ratio)).unwrap();
        let file = hdf5_metno::File::open(&ratio).unwrap();
        let dataset = file.dataset("/derived/result").unwrap();
        assert_eq!(
            dataset.dtype().unwrap().to_descriptor().unwrap(),
            TypeDescriptor::Float(FloatSize::U8)
        );
        let values = dataset.read_raw::<f64>().unwrap();
        assert_eq!(values[0], 0.0);
        assert!((values[1] - 1.0 / 3.0).abs() < 1e-12);
        drop(dataset);
        drop(file);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn addition_and_multiplication_keep_integer_results() {
        let (directory, a, b) = fixture();
        for (operation, name, expected) in [
            (
                Operation::Sum,
                "sum.hdf5",
                (0..12).map(|value| 2 * value + 2).collect::<Vec<i64>>(),
            ),
            (
                Operation::Product,
                "product.hdf5",
                (0..12)
                    .map(|value| value * (value + 2))
                    .collect::<Vec<i64>>(),
            ),
        ] {
            let output = directory.join(name);
            run(spec(&a, &b, &output, operation)).unwrap();
            let file = hdf5_metno::File::open(&output).unwrap();
            let dataset = file.dataset("/derived/result").unwrap();
            assert_eq!(dataset.shape(), vec![3, 4]);
            assert_eq!(
                dataset.dtype().unwrap().to_descriptor().unwrap(),
                TypeDescriptor::Integer(IntSize::U8)
            );
            assert_eq!(dataset.read_raw::<i64>().unwrap(), expected);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn addition_and_multiplication_keep_float32_precision() {
        let (directory, a, b) = fixture();
        for (operation, name, expected) in [
            (Operation::Sum, "float_sum.hdf5", 4.0_f32),
            (Operation::Product, "float_product.hdf5", 3.0_f32),
        ] {
            let output = directory.join(name);
            let mut job = spec(&a, &b, &output, operation);
            job.left_path = "/float32".into();
            job.right = Some((b.clone(), "/float32".into()));
            run(job).unwrap();
            let file = hdf5_metno::File::open(&output).unwrap();
            let dataset = file.dataset("/derived/result").unwrap();
            assert_eq!(
                dataset.dtype().unwrap().to_descriptor().unwrap(),
                TypeDescriptor::Float(FloatSize::U4)
            );
            assert_eq!(dataset.read_raw::<f32>().unwrap(), vec![expected; 12]);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn integer_arithmetic_rejects_results_outside_i64() {
        let (directory, a, b) = fixture();
        for (path, value) in [(&a, i64::MAX), (&b, 2_i64)] {
            let file = hdf5_metno::File::open_rw(path).unwrap();
            file.new_dataset::<i64>()
                .shape([1])
                .create("large")
                .unwrap()
                .write_raw(&[value])
                .unwrap();
        }
        for operation in [Operation::Sum, Operation::Product] {
            let output = directory.join(format!("overflow_{operation:?}.hdf5"));
            let mut job = spec(&a, &b, &output, operation);
            job.left_path = "/large".into();
            job.right = Some((b.clone(), "/large".into()));
            assert!(run(job).unwrap_err().contains("i64 range"));
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn floating_point_results_keep_the_higher_input_precision() {
        let (directory, a, b) = fixture();
        let f32_output = directory.join("f32.hdf5");
        let mut job = spec(&a, &b, &f32_output, Operation::Difference);
        job.left_path = "/float32".into();
        job.right = Some((b.clone(), "/float32".into()));
        run(job.clone()).unwrap();
        let file = hdf5_metno::File::open(&f32_output).unwrap();
        assert_eq!(
            file.dataset("/derived/result")
                .unwrap()
                .dtype()
                .unwrap()
                .to_descriptor()
                .unwrap(),
            TypeDescriptor::Float(FloatSize::U4)
        );
        drop(file);
        job.destination_file = directory.join("f64.hdf5");
        job.right = Some((b.clone(), "/float64".into()));
        run(job).unwrap();
        let file = hdf5_metno::File::open(directory.join("f64.hdf5")).unwrap();
        assert_eq!(
            file.dataset("/derived/result")
                .unwrap()
                .dtype()
                .unwrap()
                .to_descriptor()
                .unwrap(),
            TypeDescriptor::Float(FloatSize::U8)
        );
        drop(file);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn existing_output_paths_are_not_replaced() {
        let (directory, a, b) = fixture();
        let mut job = spec(&a, &b, &a, Operation::Slice);
        job.destination = Destination::A;
        job.output_path = "/x".into();
        assert!(run(job).unwrap_err().contains("already exists"));
        let file = hdf5_metno::File::open(&a).unwrap();
        assert_eq!(file.dataset("/x").unwrap().read_raw::<i16>().unwrap()[0], 0);
        drop(file);
        assert!(run(spec(&a, &b, &a, Operation::Slice))
            .unwrap_err()
            .contains("already exists"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn result_can_be_added_to_an_existing_file() {
        let (directory, a, b) = fixture();
        let mut held_reader = File::open(&a).unwrap();
        held_reader.release_for_edit().unwrap();
        let mut job = spec(&a, &b, &a, Operation::Difference);
        job.destination = Destination::A;
        run(job).unwrap();
        drop(held_reader);
        let file = hdf5_metno::File::open(&a).unwrap();
        assert_eq!(
            file.dataset("/derived/result")
                .unwrap()
                .read_raw::<i64>()
                .unwrap(),
            vec![-2; 12]
        );
        assert_eq!(file.dataset("/x").unwrap().read_raw::<i16>().unwrap()[0], 0);
        drop(file);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn plugin_controls_render_with_independent_state() {
        let (directory, a, b) = fixture();
        let paths = [a, b];
        let mut files = paths.each_ref().map(|path| File::open(path).unwrap());
        let settings = Settings::load().unwrap();
        let browser = DatasetBrowser::new(&[]);
        let mut picker_request = None;
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([1000.0, 700.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut lab = PluginLab::default();
        let ui = context.frame();
        ui.window("Dataset manipulation")
            .size([900.0, 620.0], Condition::Always)
            .build(|| {
                assert!(draw(
                    &mut lab,
                    ui,
                    PluginContext {
                        files: &mut files,
                        file_paths: &paths,
                        settings: &settings,
                        selection: None,
                        write_allowed: [true, true],
                        browser: &browser,
                        picker_request: &mut picker_request,
                    }
                )
                .is_none());
            });
        context.end_frame();
        lab.dataset_manipulation.left = 1;
        let selected = lab.dataset_manipulation.choices[1].path.clone();
        lab.dataset_manipulation.invalidate();
        let ui = context.frame();
        ui.window("Dataset manipulation")
            .size([900.0, 620.0], Condition::Always)
            .build(|| {
                draw(
                    &mut lab,
                    ui,
                    PluginContext {
                        files: &mut files,
                        file_paths: &paths,
                        settings: &settings,
                        selection: None,
                        write_allowed: [true, true],
                        browser: &browser,
                        picker_request: &mut picker_request,
                    },
                );
            });
        assert_eq!(
            lab.dataset_manipulation.choices[lab.dataset_manipulation.left].path,
            selected
        );
        context.end_frame();
        drop(files);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
