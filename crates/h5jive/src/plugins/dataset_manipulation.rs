use super::*;
use h5jive_data::{
    dataset_path_parts, execute_dataset_operation, AxisRange, DatasetOperation as Operation,
    DatasetOperationRequest, DatasetOutputTarget,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Destination {
    #[default]
    A,
    B,
    NewFile,
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
    pub fn poll(&mut self, notices: &mut NoticeCenter) -> bool {
        let mut updated = false;
        if let Some((destination, result)) = self.job.poll("Dataset operation worker stopped") {
            self.reload = destination != Destination::NewFile;
            match result {
                Ok(message) => notices.success(message),
                Err(error) => notices.error(format!("Could not save dataset: {error}")),
            }
            updated = true;
        }
        self.job.is_busy() || updated
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_busy()
    }

    pub fn active_task_labels(&self) -> Vec<&'static str> {
        if self.job.is_busy() {
            vec!["dataset write"]
        } else {
            Vec::new()
        }
    }
    pub fn take_reload(&mut self) -> bool {
        std::mem::take(&mut self.reload)
    }
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
        let _ = segmented_control(
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
        );
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
                let destination = match state.destination {
                    Destination::A | Destination::B => {
                        DatasetOutputTarget::Existing(destination_file)
                    }
                    Destination::NewFile => DatasetOutputTarget::NewFile(destination_file),
                };
                Ok(DatasetOperationRequest {
                    left_file: context.file_paths[left.file].clone(),
                    left_path: left.path.clone(),
                    right,
                    operation: state.operation,
                    ranges,
                    destination,
                    output_path: state.output_path.trim().to_owned(),
                    chunk_elements: context.settings.dataset_manipulation_chunk_elements,
                })
            });
            match spec {
                Ok(spec) => {
                    let destination = state.destination;
                    let release = match destination {
                        Destination::A | Destination::B => {
                            let slot = if destination == Destination::A { 0 } else { 1 };
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
                            state
                                .job
                                .start(destination, move |_| execute_dataset_operation(&spec));
                        }
                        Err(error) => context.notices.error(error),
                    }
                }
                Err(error) => context.notices.error(error),
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};
    use hdf5_metno::types::{FloatSize, IntSize, TypeDescriptor};
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

    fn spec(a: &Path, b: &Path, output: &Path, operation: Operation) -> DatasetOperationRequest {
        DatasetOperationRequest {
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
            destination: DatasetOutputTarget::NewFile(output.to_owned()),
            output_path: "/derived/result".into(),
            chunk_elements: 3,
        }
    }

    #[test]
    fn slice_preserves_integer_type_and_selected_indexes() {
        let (directory, a, b) = fixture();
        let output = directory.join("slice.hdf5");
        execute_dataset_operation(&spec(&a, &b, &output, Operation::Slice)).unwrap();
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
        execute_dataset_operation(&spec(&a, &b, &difference, Operation::Difference)).unwrap();
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
        execute_dataset_operation(&spec(&a, &b, &ratio, Operation::Ratio)).unwrap();
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
            execute_dataset_operation(&spec(&a, &b, &output, operation)).unwrap();
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
            execute_dataset_operation(&job).unwrap();
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
            assert!(execute_dataset_operation(&job)
                .unwrap_err()
                .contains("i64 range"));
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
        execute_dataset_operation(&job).unwrap();
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
        job.destination = DatasetOutputTarget::NewFile(directory.join("f64.hdf5"));
        job.right = Some((b.clone(), "/float64".into()));
        execute_dataset_operation(&job).unwrap();
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
        job.destination = DatasetOutputTarget::Existing(a.to_owned());
        job.output_path = "/x".into();
        assert!(execute_dataset_operation(&job)
            .unwrap_err()
            .contains("already exists"));
        let file = hdf5_metno::File::open(&a).unwrap();
        assert_eq!(file.dataset("/x").unwrap().read_raw::<i16>().unwrap()[0], 0);
        drop(file);
        assert!(
            execute_dataset_operation(&spec(&a, &b, &a, Operation::Slice))
                .unwrap_err()
                .contains("already exists")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn result_can_be_added_to_an_existing_file() {
        let (directory, a, b) = fixture();
        let mut held_reader = File::open(&a).unwrap();
        held_reader.release_for_edit().unwrap();
        let mut job = spec(&a, &b, &a, Operation::Difference);
        job.destination = DatasetOutputTarget::Existing(a.to_owned());
        execute_dataset_operation(&job).unwrap();
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
                        notices: &mut NoticeCenter::default(),
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
                        notices: &mut NoticeCenter::default(),
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
