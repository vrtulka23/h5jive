use super::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc,
};
use std::thread;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DatasetChoice {
    file: usize,
    path: String,
    shape: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Operation {
    #[default]
    Difference,
    Ratio,
}

impl Operation {
    fn label(self) -> &'static str {
        match self {
            Self::Difference => "Left − Right",
            Self::Ratio => "Left ÷ Right",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewKey {
    left: DatasetChoice,
    right: DatasetChoice,
    slice: TableSlice,
    operation: Operation,
    grid: [usize; 2],
}

struct Preview {
    values: Vec<f64>,
    left_values: Vec<f64>,
    right_values: Vec<f64>,
    plotted: Vec<f64>,
    range: Option<(f64, f64)>,
    invalid: usize,
}

#[derive(Default)]
pub(super) struct State {
    choices: Option<Arc<Vec<DatasetChoice>>>,
    left: Option<DatasetChoice>,
    right: Option<DatasetChoice>,
    left_search: String,
    right_search: String,
    slice: Option<TableSlice>,
    operation: Operation,
    preview: Option<(PreviewKey, Result<Preview, String>)>,
    job: Option<(PreviewKey, mpsc::Receiver<Result<Preview, String>>)>,
    generation: Arc<AtomicU64>,
    needs_redraw: bool,
}

impl State {
    pub fn invalidate(&mut self) {
        self.choices = None;
        self.left = None;
        self.right = None;
        self.slice = None;
        self.left_search.clear();
        self.right_search.clear();
        self.invalidate_preview();
    }

    fn invalidate_preview(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.preview = None;
        self.job = None;
        self.needs_redraw = true;
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.needs_redraw)
    }

    pub fn poll(&mut self) {
        let Some((key, receiver)) = &self.job else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Value comparison worker stopped unexpectedly".into())
            }
        };
        self.preview = Some((key.clone(), result));
        self.job = None;
        self.needs_redraw = true;
    }

    fn start(&mut self, paths: &[PathBuf], key: PreviewKey) {
        let (Some(left_path), Some(right_path)) =
            (paths.get(key.left.file), paths.get(key.right.file))
        else {
            self.preview = Some((key, Err("Selected file is no longer loaded".into())));
            return;
        };
        let (left_path, right_path) = (left_path.clone(), right_path.clone());
        let generation = Arc::clone(&self.generation);
        let expected = generation.load(Ordering::Relaxed);
        let worker_key = key.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let cancelled = || generation.load(Ordering::Relaxed) != expected;
            let result = load_preview(&left_path, &right_path, &worker_key, &cancelled);
            if !cancelled() {
                let _ = sender.send(result);
            }
        });
        self.job = Some((key, receiver));
    }
}

fn collect_choices(files: &[File]) -> Vec<DatasetChoice> {
    fn visit(file: usize, entry: &Entry, choices: &mut Vec<DatasetChoice>) {
        if let Kind::Dataset {
            shape,
            numeric: true,
        } = &entry.kind
        {
            if shape.len() >= 2 && shape.iter().all(|&length| length > 0) {
                choices.push(DatasetChoice {
                    file,
                    path: entry.path.clone(),
                    shape: shape.clone(),
                });
            }
        }
        for child in &entry.children {
            visit(file, child, choices);
        }
    }
    let mut choices = Vec::new();
    for (file, source) in files.iter().take(2).enumerate() {
        visit(file, &source.tree, &mut choices);
    }
    choices.sort_by(|a, b| (a.file, &a.path).cmp(&(b.file, &b.path)));
    choices
}

fn compatible<'a>(choices: &'a [DatasetChoice], first: &DatasetChoice) -> Vec<&'a DatasetChoice> {
    let mut matches = choices
        .iter()
        .filter(|choice| choice.shape == first.shape)
        .collect::<Vec<_>>();
    matches.sort_by_key(|choice| {
        (
            choice.path != first.path,
            choice.file == first.file,
            choice.file,
            &choice.path,
        )
    });
    matches
}

fn default_pair(
    state: &mut State,
    choices: &[DatasetChoice],
    core: Option<&Selection>,
    default: PinnedDefault,
) {
    if state
        .left
        .as_ref()
        .is_none_or(|left| !choices.contains(left))
    {
        state.left = core
            .and_then(|selection| {
                choices
                    .iter()
                    .find(|choice| choice.file == selection.file && choice.path == selection.path)
            })
            .or_else(|| choices.first())
            .cloned();
        state.right = None;
        state.slice = None;
        state.invalidate_preview();
    }
    let Some(left) = state.left.clone() else {
        return;
    };
    let candidates = compatible(choices, &left);
    if state
        .right
        .as_ref()
        .is_none_or(|right| !candidates.contains(&right))
    {
        state.right = candidates.first().map(|choice| (*choice).clone());
        state.invalidate_preview();
    }
    if state
        .slice
        .as_ref()
        .is_none_or(|slice| slice.pinned.len() != left.shape.len())
    {
        state.slice = Some(TableSlice::new(&left.shape, default));
        state.invalidate_preview();
    }
}

fn compare_cell(
    left: f64,
    right: f64,
    left_fill: Option<f64>,
    right_fill: Option<f64>,
    operation: Operation,
) -> f64 {
    if finite_value(left, left_fill).is_none() || finite_value(right, right_fill).is_none() {
        return f64::NAN;
    }
    let result = match operation {
        Operation::Difference => left - right,
        Operation::Ratio if right != 0.0 => left / right,
        Operation::Ratio => return f64::NAN,
    };
    if result.is_finite() {
        result
    } else {
        f64::NAN
    }
}

fn comparison_grid(shape: &[usize], slice: &TableSlice, max_cells: usize) -> [usize; 2] {
    let mut grid = [slice.rows(shape).min(512), slice.columns(shape).min(512)];
    if grid[0].saturating_mul(grid[1]) > max_cells {
        let scale = (max_cells as f64 / grid[0] as f64 / grid[1] as f64).sqrt();
        grid[0] = ((grid[0] as f64 * scale).floor() as usize).max(1);
        grid[1] = ((grid[1] as f64 * scale).floor() as usize).max(1);
    }
    grid
}

fn load_preview(
    left_path: &std::path::Path,
    right_path: &std::path::Path,
    key: &PreviewKey,
    cancelled: &dyn Fn() -> bool,
) -> Result<Preview, String> {
    if cancelled() {
        return Err("Comparison cancelled".into());
    }
    let left = open_dataset_reader(left_path)?;
    let right = open_dataset_reader(right_path)?;
    let left_info = left
        .dataset_info(&key.left.path)
        .map_err(|e| e.to_string())?;
    let right_info = right
        .dataset_info(&key.right.path)
        .map_err(|e| e.to_string())?;
    if !left_info.numeric
        || !right_info.numeric
        || left_info.shape != right_info.shape
        || left_info.shape != key.left.shape
    {
        return Err("The selected datasets must be numeric and have identical shapes".into());
    }
    let a = load_heatmap_overview_with_cancel(
        &left,
        &key.left.path,
        &left_info.shape,
        &key.slice,
        key.grid,
        cancelled,
    )?;
    let b = load_heatmap_overview_with_cancel(
        &right,
        &key.right.path,
        &right_info.shape,
        &key.slice,
        key.grid,
        cancelled,
    )?;
    let values = a
        .values
        .iter()
        .zip(&b.values)
        .map(|(&left, &right)| compare_cell(left, right, a.fill_value, b.fill_value, key.operation))
        .collect::<Vec<_>>();
    let mut range = None;
    extend_finite_range(&mut range, &values, None);
    if let Some((min, max)) = &mut range {
        if key.operation == Operation::Difference {
            let magnitude = min.abs().max(max.abs());
            let magnitude = if magnitude == 0.0 { 1.0 } else { magnitude };
            *min = -magnitude;
            *max = magnitude;
        } else if *min == *max {
            let margin = min.abs().max(1.0) * 0.05;
            *min -= margin;
            *max += margin;
        }
    }
    let replacement = range.map_or(0.0, |(min, _)| min);
    let mut plotted = Vec::with_capacity(values.len());
    for row in (0..key.grid[0]).rev() {
        for column in 0..key.grid[1] {
            let value = values[row * key.grid[1] + column];
            plotted.push(if value.is_finite() {
                value
            } else {
                replacement
            });
        }
    }
    let invalid = values.iter().filter(|value| !value.is_finite()).count();
    Ok(Preview {
        values,
        left_values: a.values,
        right_values: b.values,
        plotted,
        range,
        invalid,
    })
}

pub(super) fn draw_icon(draw: &DrawListMut<'_>, min: [f32; 2], max: [f32; 2], color: [f32; 4]) {
    let x = (min[0] + max[0]) * 0.5;
    let y = (min[1] + max[1]) * 0.5;
    for dx in [-12.0, 4.0] {
        draw.add_rect([x + dx, y - 11.0], [x + dx + 8.0, y + 11.0], color)
            .thickness(1.6)
            .build();
    }
    draw.add_line([x - 1.0, y - 2.0], [x + 1.0, y - 2.0], color)
        .thickness(1.7)
        .build();
    draw.add_circle([x, y + 5.0], 1.2, color)
        .filled(true)
        .build();
}

fn picker(
    ui: &Ui,
    id: &str,
    title: &str,
    choices: &[&DatasetChoice],
    selected: Option<&DatasetChoice>,
    search: &mut String,
) -> Option<DatasetChoice> {
    ui.text(title);
    let preview = selected.map_or_else(
        || "Select a dataset".to_owned(),
        |choice| {
            format!(
                "{} · {}  {:?}",
                if choice.file == 0 { "A" } else { "B" },
                choice.path,
                choice.shape
            )
        },
    );
    ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
    let mut picked = None;
    if let Some(_combo) = ui.begin_combo(format!("##dataset_select_{id}"), &preview) {
        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
        ui.input_text(format!("Search##{id}"), search).build();
        let query = search.trim().to_lowercase();
        let visible = choices
            .iter()
            .filter(|choice| {
                query.is_empty()
                    || choice.path.to_lowercase().contains(&query)
                    || (if choice.file == 0 { "a" } else { "b" }).contains(&query)
            })
            .copied()
            .collect::<Vec<_>>();
        ui.text_disabled(format!("{} compatible datasets", visible.len()));
        ui.child_window(format!("options##{id}"))
            .size([0.0, 220.0])
            .build(ui, || {
                if visible.is_empty() {
                    ui.text_disabled("No datasets match this search.");
                }
                let mut clipper = ListClipper::new(visible.len()).begin(ui);
                while clipper.step() {
                    for index in clipper.display_start()..clipper.display_end() {
                        let choice = visible[index];
                        let file = if choice.file == 0 { "A" } else { "B" };
                        let label =
                            format!("{file} · {}  {:?}##{id}_{index}", choice.path, choice.shape);
                        if ui
                            .selectable_config(label)
                            .selected(selected == Some(choice))
                            .build()
                        {
                            picked = Some(choice.clone());
                        }
                        if ui.is_item_hovered() {
                            ui.set_item_tooltip(format!(
                                "File {file}: {}\nShape {:?}",
                                choice.path, choice.shape
                            ));
                        }
                    }
                }
            });
    }
    picked
}

fn draw_preview(
    ui: &Ui,
    plot_ui: &PlotUi<'_>,
    key: &PreviewKey,
    preview: &Preview,
    palette: &Palette,
    decimals: usize,
) {
    let Some((min, max)) = preview.range else {
        ui.text_wrapped("No finite comparison values in this slice. Fill values, NaNs, infinities, and division by zero are excluded.");
        return;
    };
    let rows = key.slice.rows(&key.left.shape);
    let columns = key.slice.columns(&key.left.shape);
    let available = ui.content_region_avail();
    let plot_width = (available[0] - 75.0).max(100.0);
    let plot_height = available[1].max(180.0);
    let map = if key.operation == Operation::Difference {
        Colormap::RdBu
    } else {
        Colormap::Viridis
    };
    let _colormap = plot_ui.push_colormap(map);
    let title = format!(
        "##value-comparison-{}-{}-{}-{}-{:?}-{:?}",
        key.left.file, key.left.path, key.right.file, key.right.path, key.slice, key.operation
    );
    if let Some(_plot) = plot_ui.begin_plot_with_size(&title, [plot_width, plot_height]) {
        plot_ui.setup_axes(
            Some("Column index"),
            Some("Row index"),
            AxisFlags::NONE,
            AxisFlags::NONE,
        );
        plot_ui.setup_axes_limits(0.0, columns as f64, 0.0, rows as f64, PlotCond::Once);
        plot_ui.setup_finish();
        plot_heatmap_grid(
            plot_ui,
            "comparison",
            &preview.plotted,
            key.grid,
            (min, max),
            [0.0, 0.0, columns as f64, rows as f64],
        );
        if preview.invalid > 0 {
            let draw = ui.get_window_draw_list();
            let plot_pos = plot_ui.plot_pos();
            let plot_size = plot_ui.plot_size();
            let _clip = draw.push_clip_rect(
                plot_pos,
                [plot_pos[0] + plot_size[0], plot_pos[1] + plot_size[1]],
                true,
            );
            for row in 0..key.grid[0] {
                for column in 0..key.grid[1] {
                    if preview.values[row * key.grid[1] + column].is_finite() {
                        continue;
                    }
                    let x0 = column as f64 * columns as f64 / key.grid[1] as f64;
                    let x1 = (column + 1) as f64 * columns as f64 / key.grid[1] as f64;
                    let y0 = row as f64 * rows as f64 / key.grid[0] as f64;
                    let y1 = (row + 1) as f64 * rows as f64 / key.grid[0] as f64;
                    let first = plot_ui.plot_to_pixels_axes(
                        ImPlotPoint { x: x0, y: y1 },
                        XAxis::X1,
                        YAxis::Y1,
                    );
                    let last = plot_ui.plot_to_pixels_axes(
                        ImPlotPoint { x: x1, y: y0 },
                        XAxis::X1,
                        YAxis::Y1,
                    );
                    draw.add_rect(first, last, palette.nan).filled(true).build();
                }
            }
        }
        if plot_ui.is_plot_hovered() {
            let point = plot_ui.get_plot_mouse_pos_axes(XAxis::X1, YAxis::Y1);
            if point.x >= 0.0 && point.y >= 0.0 && point.x < columns as f64 && point.y < rows as f64
            {
                let row_bin = ((point.y / rows as f64) * key.grid[0] as f64).floor() as usize;
                let col_bin = ((point.x / columns as f64) * key.grid[1] as f64).floor() as usize;
                let index = row_bin * key.grid[1] + col_bin;
                let native_row = overview_sample_index(row_bin, rows, key.grid[0]);
                let native_col = overview_sample_index(col_bin, columns, key.grid[1]);
                ui.set_tooltip(format!(
                    "Row {native_row}, column {native_col}\nLeft: {}\nRight: {}\n{}: {}",
                    format_value(preview.left_values[index], decimals),
                    format_value(preview.right_values[index], decimals),
                    key.operation.label(),
                    format_value(preview.values[index], decimals)
                ));
            }
        }
    }
    ui.same_line_with_spacing(0.0, 6.0);
    plot_ui.colormap_scale("##comparison_scale", min, max, plot_height, map);
}

pub(super) fn draw(
    lab: &mut PluginLab,
    ui: &Ui,
    plot_ui: &PlotUi<'_>,
    context: PluginContext<'_>,
) -> Option<Selection> {
    let state = &mut lab.value_comparison;
    let choices = Arc::clone(
        state
            .choices
            .get_or_insert_with(|| Arc::new(collect_choices(context.files))),
    );
    if choices.is_empty() {
        ui.text_wrapped("Load a file with a numeric dataset of at least two dimensions. The second dataset may come from the same file or the other file.");
        return None;
    }
    default_pair(
        state,
        &choices,
        context.selection,
        context.settings.pinned_default,
    );
    let all = choices.iter().collect::<Vec<_>>();
    let width = ((ui.content_region_avail()[0] - 8.0) * 0.5).max(1.0);
    let mut new_left = None;
    ui.child_window("value_left_picker")
        .size([width, 72.0])
        .build(ui, || {
            new_left = picker(
                ui,
                "left",
                "Left dataset · A or B",
                &all,
                state.left.as_ref(),
                &mut state.left_search,
            );
        });
    ui.same_line_with_spacing(0.0, 8.0);
    if let Some(left) = new_left {
        state.left = Some(left);
        state.right = None;
        state.slice = None;
        state.right_search.clear();
        state.invalidate_preview();
        default_pair(
            state,
            &choices,
            context.selection,
            context.settings.pinned_default,
        );
    }
    let left = state.left.as_ref().unwrap().clone();
    let candidates = compatible(&choices, &left);
    let mut new_right = None;
    ui.child_window("value_right_picker")
        .size([0.0, 72.0])
        .build(ui, || {
            new_right = picker(
                ui,
                "right",
                "Right dataset · matching shape",
                &candidates,
                state.right.as_ref(),
                &mut state.right_search,
            );
        });
    if let Some(right) = new_right {
        state.right = Some(right);
        state.invalidate_preview();
    }
    let Some(right) = state.right.as_ref().cloned() else {
        return None;
    };
    ui.text_wrapped(format!(
        "Left: {} {}    Right: {} {}    Shape {:?}",
        if left.file == 0 { "A" } else { "B" },
        left.path,
        if right.file == 0 { "A" } else { "B" },
        right.path,
        left.shape
    ));
    ui.spacing();
    for operation in [Operation::Difference, Operation::Ratio] {
        if operation == Operation::Ratio {
            ui.same_line_with_spacing(0.0, 8.0);
        }
        let active = state.operation == operation;
        let _button = ui.push_style_color(
            StyleColor::Button,
            if active {
                context.settings.palette.accent
            } else {
                context.settings.palette.panel
            },
        );
        if ui.button(operation.label()) && !active {
            state.operation = operation;
            state.invalidate_preview();
        }
    }
    let slice = state.slice.as_mut().unwrap();
    let labels = axis_option_labels(&[], &left.shape);
    let mut row = slice.row_axis.unwrap_or(0);
    ui.set_next_item_width(150.0);
    if ui.combo_simple_string("Rows##value_rows", &mut row, &labels) {
        slice.row_axis = Some(row);
        if slice.column_axis == row {
            slice.column_axis = (0..left.shape.len()).find(|&axis| axis != row).unwrap();
        }
        state.invalidate_preview();
    }
    ui.same_line_with_spacing(0.0, 8.0);
    let slice = state.slice.as_mut().unwrap();
    ui.set_next_item_width(150.0);
    if ui.combo_simple_string("Columns##value_columns", &mut slice.column_axis, &labels) {
        if slice.row_axis == Some(slice.column_axis) {
            slice.row_axis = (0..left.shape.len()).find(|&axis| axis != slice.column_axis);
        }
        state.invalidate_preview();
    }
    let slice = state.slice.as_mut().unwrap();
    let mut pins_changed = false;
    for (axis, &length) in left.shape.iter().enumerate() {
        if slice.row_axis == Some(axis) || slice.column_axis == axis {
            continue;
        }
        let mut pin = slice.pinned[axis].min(i32::MAX as usize) as i32;
        ui.set_next_item_width(180.0);
        if ui.slider(
            format!("Pinned dimension {axis}##value_pin_{axis}"),
            0,
            length.saturating_sub(1).min(i32::MAX as usize) as i32,
            &mut pin,
        ) {
            slice.pinned[axis] = pin as usize;
            pins_changed = true;
        }
    }
    if pins_changed {
        state.invalidate_preview();
    }
    let slice = state.slice.as_ref().unwrap().clone();
    let grid = comparison_grid(
        &left.shape,
        &slice,
        context.settings.value_comparison_max_cells,
    );
    let key = PreviewKey {
        left,
        right,
        slice,
        operation: state.operation,
        grid,
    };
    if state
        .preview
        .as_ref()
        .is_none_or(|(cached, _)| cached != &key)
        && state
            .job
            .as_ref()
            .is_none_or(|(running, _)| running != &key)
    {
        state.invalidate_preview();
        state.start(context.file_paths, key.clone());
    }
    ui.separator();
    if grid
        != [
            key.slice.rows(&key.left.shape),
            key.slice.columns(&key.left.shape),
        ]
    {
        ui.text_disabled(format!(
            "Sampled 2D preview: {} × {} cells; matching source positions are used on both sides.",
            grid[0], grid[1]
        ));
    }
    match state.preview.as_ref().filter(|(cached, _)| cached == &key) {
        Some((_, Ok(preview))) => {
            ui.text(format!(
                "{} · {} valid · {} excluded",
                key.operation.label(),
                preview.values.len() - preview.invalid,
                preview.invalid
            ));
            draw_preview(
                ui,
                plot_ui,
                &key,
                preview,
                &context.settings.palette,
                context.settings.display_decimals,
            );
        }
        Some((_, Err(error))) => ui.text_wrapped(format!("Could not compare datasets: {error}")),
        None => ui.text("Loading comparison preview..."),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    fn demos() -> ([PathBuf; 2], [File; 2]) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/value_comparison");
        let paths = ["value_demo_a.hdf5", "value_demo_b.hdf5"].map(|name| root.join(name));
        let files = paths.each_ref().map(|path| File::open(path).unwrap());
        (paths, files)
    }

    #[test]
    fn matching_picker_offers_cross_file_and_same_file_pairs_only() {
        let (_, files) = demos();
        let choices = collect_choices(&files);
        let first = choices
            .iter()
            .find(|choice| choice.file == 0 && choice.path == "/field/plane")
            .unwrap();
        let matching = compatible(&choices, first);
        assert!(matching
            .iter()
            .any(|choice| choice.file == 1 && choice.path == "/field/plane"));
        assert!(matching
            .iter()
            .any(|choice| choice.file == 0 && choice.path == "/field/reference"));
        assert!(!matching
            .iter()
            .any(|choice| choice.file == 0 && choice.path == "/field/cube"));
        let other = choices
            .iter()
            .find(|choice| choice.file == 0 && choice.path == "/field/other_shape")
            .unwrap();
        assert!(!compatible(&choices, other)
            .iter()
            .any(|choice| choice.file == 1 && choice.path == "/field/other_shape"));
    }

    #[test]
    fn demo_preview_compares_same_positions_and_pinned_planes() {
        let (paths, files) = demos();
        let choices = collect_choices(&files);
        let pick = |file, path| {
            choices
                .iter()
                .find(|choice| choice.file == file && choice.path == path)
                .unwrap()
                .clone()
        };
        let left = pick(0, "/field/plane");
        let right = pick(1, "/field/plane");
        let slice = TableSlice::new(&left.shape, PinnedDefault::Start);
        let mut key = PreviewKey {
            left,
            right,
            slice,
            operation: Operation::Difference,
            grid: [3, 4],
        };
        let preview = load_preview(&paths[0], &paths[1], &key, &|| false).unwrap();
        assert_eq!(preview.values, vec![-2.0; 12]);
        assert_eq!(preview.invalid, 0);
        key.operation = Operation::Ratio;
        let preview = load_preview(&paths[0], &paths[1], &key, &|| false).unwrap();
        assert!((preview.values[0] - 1.0 / 3.0).abs() < 1e-12);
        key.left = pick(0, "/field/cube");
        key.right = pick(1, "/field/cube");
        key.slice = TableSlice::new(&key.left.shape, PinnedDefault::Start);
        key.slice.pinned[0] = 1;
        key.slice.row_axis = Some(1);
        key.slice.column_axis = 2;
        key.operation = Operation::Difference;
        let preview = load_preview(&paths[0], &paths[1], &key, &|| false).unwrap();
        assert_eq!(preview.values, vec![-3.0; 12]);
    }

    #[test]
    fn arithmetic_excludes_fill_and_zero_denominator() {
        assert_eq!(
            compare_cell(8.0, 2.0, None, None, Operation::Difference),
            6.0
        );
        assert_eq!(compare_cell(8.0, 2.0, None, None, Operation::Ratio), 4.0);
        assert!(compare_cell(8.0, 0.0, None, None, Operation::Ratio).is_nan());
        assert!(compare_cell(-999.0, 2.0, Some(-999.0), None, Operation::Difference).is_nan());
        assert!(compare_cell(f64::NAN, 2.0, None, None, Operation::Difference).is_nan());
    }

    #[test]
    fn plugin_heatmap_renders_with_its_own_state() {
        let (paths, files) = demos();
        let choices = collect_choices(&files);
        let left = choices
            .iter()
            .find(|choice| choice.file == 0 && choice.path == "/field/plane")
            .unwrap()
            .clone();
        let right = choices
            .iter()
            .find(|choice| choice.file == 1 && choice.path == "/field/plane")
            .unwrap()
            .clone();
        let slice = TableSlice::new(&left.shape, PinnedDefault::Start);
        let key = PreviewKey {
            left: left.clone(),
            right: right.clone(),
            slice,
            operation: Operation::Difference,
            grid: [3, 4],
        };
        let preview = load_preview(&paths[0], &paths[1], &key, &|| false).unwrap();
        let settings = Settings::load().unwrap();
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([1100.0, 800.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let plot_ctx = PlotContext::create(&context);
        let mut lab = PluginLab::default();
        lab.selected = 1;
        lab.value_comparison.left = Some(left);
        lab.value_comparison.right = Some(right);
        lab.value_comparison.slice = Some(key.slice.clone());
        lab.value_comparison.preview = Some((key, Ok(preview)));
        let ui = context.frame();
        let plot_ui = plot_ctx.get_plot_ui(ui);
        ui.window("Plugin Lab")
            .size([1000.0, 720.0], Condition::Always)
            .build(|| {
                assert!(lab
                    .draw(ui, &plot_ui, &files, &paths, &settings, None)
                    .is_none());
            });
        context.end_frame();
    }
}
