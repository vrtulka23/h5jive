use super::*;
use dear_app::imgui::MouseButton;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn draw_icon(draw: &DrawListMut<'_>, min: [f32; 2], max: [f32; 2], color: [f32; 4]) {
    let width = max[0] - min[0];
    let height = max[1] - min[1];
    let side = width.min(height).min(38.0);
    let origin = [
        min[0] + (width - side) * 0.5,
        min[1] + (height - side) * 0.5,
    ];
    let p = |x: f32, y: f32| [origin[0] + side * x, origin[1] + side * y];
    for (left, right) in [(0.12, 0.43), (0.57, 0.88)] {
        draw.add_rect(p(left, 0.17), p(right, 0.83), color)
            .rounding(2.0)
            .thickness(1.5)
            .build();
        draw.add_line(p(left + 0.07, 0.38), p(right - 0.07, 0.38), color)
            .thickness(1.5)
            .build();
        draw.add_line(p(left + 0.07, 0.60), p(right - 0.07, 0.60), color)
            .thickness(1.5)
            .build();
    }
    draw.add_line(p(0.46, 0.43), p(0.54, 0.43), color)
        .thickness(1.7)
        .build();
    draw.add_line(p(0.46, 0.57), p(0.54, 0.57), color)
        .thickness(1.7)
        .build();
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathKind {
    Group,
    Dataset { shape: Vec<usize> },
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EntrySummary {
    kind: PathKind,
    datatype: Option<String>,
    attributes: BTreeMap<String, String>,
    errors: Vec<String>,
}

impl EntrySummary {
    fn detail(&self) -> String {
        match (&self.kind, &self.datatype) {
            (PathKind::Dataset { shape }, Some(datatype)) => {
                format!("Dataset {shape:?} · {datatype}")
            }
            _ => self.kind.label(),
        }
    }

    fn attribute_line(&self) -> String {
        if self.attributes.is_empty() {
            "No attributes".into()
        } else {
            self.attributes
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("  ·  ")
        }
    }
}

impl PathKind {
    fn from_entry(entry: &Entry) -> Self {
        match &entry.kind {
            Kind::Group => Self::Group,
            Kind::Dataset { shape, .. } => Self::Dataset {
                shape: shape.clone(),
            },
            Kind::Other => Self::Other,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Group => "Group".into(),
            Self::Dataset { shape, .. } => format!("Dataset {shape:?}"),
            Self::Other => "Other".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Same,
    Different,
    OnlyA,
    OnlyB,
    Unavailable,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Same => "Same",
            Self::Different => "Different",
            Self::OnlyA => "Only A",
            Self::OnlyB => "Only B",
            Self::Unavailable => "Metadata unavailable",
        }
    }
}

fn status_color(status: Status, palette: &Palette) -> [f32; 4] {
    match status {
        Status::Same => palette.heatmap_mid,
        Status::Different => palette.positive_inf,
        Status::OnlyA => palette.accent,
        Status::OnlyB => palette.heatmap_low,
        Status::Unavailable => palette.muted,
    }
}

const STATUS_ICON_SIZE: [f32; 2] = [32.0, 28.0];

fn draw_status_icon(
    draw: &DrawListMut<'_>,
    status: Status,
    min: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
) {
    let p = |x: f32, y: f32| [min[0] + x * size[0] / 32.0, min[1] + y * size[1] / 28.0];
    let radius = size[0].min(size[1]) * 0.36;
    match status {
        Status::Same => {
            draw.add_circle(p(16.0, 14.0), radius, color)
                .thickness(1.3)
                .build();
            draw.add_line(p(10.0, 14.0), p(14.0, 18.0), color)
                .thickness(1.5)
                .build();
            draw.add_line(p(14.0, 18.0), p(22.0, 10.0), color)
                .thickness(1.5)
                .build();
        }
        Status::Different => {
            draw.add_circle(p(16.0, 14.0), radius, color)
                .thickness(1.3)
                .build();
            draw.add_line(p(16.0, 8.0), p(16.0, 15.0), color)
                .thickness(1.5)
                .build();
            draw.add_circle(p(16.0, 20.0), 1.0, color)
                .filled(true)
                .build();
        }
        Status::OnlyA | Status::OnlyB => {
            let (start, end, tip) = if status == Status::OnlyA {
                (23.0, 9.0, 9.0)
            } else {
                (9.0, 23.0, 23.0)
            };
            draw.add_line(p(start, 14.0), p(end, 14.0), color)
                .thickness(1.5)
                .build();
            let wing = if status == Status::OnlyA { 5.0 } else { -5.0 };
            draw.add_line(p(tip, 14.0), p(tip + wing, 9.0), color)
                .thickness(1.5)
                .build();
            draw.add_line(p(tip, 14.0), p(tip + wing, 19.0), color)
                .thickness(1.5)
                .build();
        }
        Status::Unavailable => {
            draw.add_circle(p(16.0, 14.0), radius, color)
                .thickness(1.3)
                .build();
            draw.add_line(p(16.0, 8.0), p(16.0, 15.0), color)
                .thickness(1.5)
                .build();
            draw.add_circle(p(16.0, 20.0), 1.0, color)
                .filled(true)
                .build();
        }
    }
}

fn center_status_cell(ui: &Ui, width: f32) {
    let offset = (ui.content_region_avail()[0] - width).max(0.0) * 0.5;
    ui.set_cursor_pos_x(ui.cursor_pos_x() + offset);
}

fn draw_entry_header(
    ui: &Ui,
    row: &Row,
    summary: Option<&EntrySummary>,
    slot: usize,
    index: usize,
    expanded: bool,
    current: Option<&Selection>,
    palette: &Palette,
    cell_background: [f32; 4],
) -> (bool, Option<Selection>) {
    let is_dataset = summary.is_some_and(|entry| matches!(entry.kind, PathKind::Dataset { .. }));
    let active = is_dataset
        && current.is_some_and(|selection| selection.file == slot && selection.path == row.path);
    let label = if summary.is_some() {
        &row.path
    } else {
        "Not present"
    };
    let column_color = if slot == 0 {
        palette.accent
    } else {
        palette.heatmap_low
    };
    let clicked = {
        let _selected = ui.push_style_color(
            StyleColor::Header,
            mix_color(cell_background, column_color, 0.24),
        );
        let _hover = ui.push_style_color(
            StyleColor::HeaderHovered,
            mix_color(cell_background, column_color, 0.36),
        );
        let _pressed = ui.push_style_color(
            StyleColor::HeaderActive,
            mix_color(cell_background, column_color, 0.52),
        );
        ui.selectable_config(format!("##path_{slot}_{index}"))
            .size([0.0, STATUS_ICON_SIZE[1]])
            .selected(active)
            .allow_double_click(true)
            .build()
    };
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let center_y = (min[1] + max[1]) * 0.5;
    let chevron_x = min[0] + 12.0;
    let draw = ui.get_window_draw_list();
    let stroke = if summary.is_some() {
        palette.text
    } else {
        palette.muted
    };
    let chevron = if expanded {
        (
            [chevron_x - 4.0, center_y - 2.0],
            [chevron_x, center_y + 2.0],
            [chevron_x + 4.0, center_y - 2.0],
        )
    } else {
        (
            [chevron_x - 2.0, center_y - 4.0],
            [chevron_x + 2.0, center_y],
            [chevron_x - 2.0, center_y + 4.0],
        )
    };
    draw.add_line(chevron.0, chevron.1, stroke)
        .thickness(1.5)
        .build();
    draw.add_line(chevron.1, chevron.2, stroke)
        .thickness(1.5)
        .build();
    draw.add_text(
        [min[0] + 27.0, center_y - ui.text_line_height() * 0.5],
        stroke,
        label,
    );
    let toggled = clicked && !ui.is_mouse_double_clicked(MouseButton::Left);
    let mut selected = None;
    if clicked && is_dataset && ui.is_mouse_double_clicked(MouseButton::Left) {
        selected = Some(Selection {
            file: slot,
            path: row.path.clone(),
        });
    }
    if ui.is_item_hovered() {
        ui.set_item_tooltip(if is_dataset {
            format!(
                "{}\nClick to expand or collapse both sides; double-click to inspect",
                row.path
            )
        } else {
            format!("{}\nClick to expand or collapse both sides", row.path)
        });
    }
    (toggled, selected)
}

fn draw_entry_detail(ui: &Ui, summary: Option<&EntrySummary>, attributes: bool, palette: &Palette) {
    let detail = summary.map_or_else(
        || {
            if attributes {
                " ".into()
            } else {
                "Not present".into()
            }
        },
        |summary| {
            if attributes {
                summary.attribute_line()
            } else {
                summary.detail()
            }
        },
    );
    ui.text_colored(palette.muted, &detail);
    if ui.is_item_hovered() {
        ui.set_item_tooltip(&detail);
    }
}

struct Row {
    path: String,
    a: Option<EntrySummary>,
    b: Option<EntrySummary>,
    status: Status,
    differences: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisplayKind {
    Header,
    Metadata,
    Attributes,
}

fn display_items<'a>(
    rows: &'a [Row],
    filter: Option<Status>,
    expanded: &BTreeSet<String>,
) -> Vec<(&'a Row, DisplayKind, usize)> {
    let mut items = Vec::new();
    for (row_index, row) in rows
        .iter()
        .filter(|row| filter.is_none_or(|status| row.status == status))
        .enumerate()
    {
        items.push((row, DisplayKind::Header, row_index));
        if expanded.contains(&row.path) {
            items.push((row, DisplayKind::Metadata, row_index));
            items.push((row, DisplayKind::Attributes, row_index));
        }
    }
    items
}

fn collect(
    file: &File,
    entry: &Entry,
    show_groups: bool,
    paths: &mut BTreeMap<String, EntrySummary>,
) {
    if entry.path != "/" && (show_groups || !matches!(entry.kind, Kind::Group)) {
        let mut summary = EntrySummary {
            kind: PathKind::from_entry(entry),
            datatype: None,
            attributes: BTreeMap::new(),
            errors: Vec::new(),
        };
        if matches!(entry.kind, Kind::Dataset { .. }) {
            match file.dataset_info(&entry.path) {
                Ok(info) => summary.datatype = Some(info.datatype),
                Err(error) => summary.errors.push(format!("Datatype: {error}")),
            }
        }
        if !matches!(entry.kind, Kind::Other) {
            match file.attributes(&entry.path) {
                Ok(attributes) => {
                    summary.attributes = attributes
                        .into_iter()
                        .map(|attribute| (attribute.name, attribute.value))
                        .collect();
                }
                Err(error) => summary.errors.push(format!("Attributes: {error}")),
            }
        }
        paths.insert(entry.path.clone(), summary);
    }
    for child in &entry.children {
        collect(file, child, show_groups, paths);
    }
}

fn compare(a: &File, b: &File, show_groups: bool) -> Vec<Row> {
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    collect(a, &a.tree, show_groups, &mut left);
    collect(b, &b.tree, show_groups, &mut right);
    let mut paths: Vec<_> = left.keys().chain(right.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            let a = left.remove(&path);
            let b = right.remove(&path);
            let mut differences = Vec::new();
            let status = match (&a, &b) {
                (Some(a), Some(b)) if !a.errors.is_empty() || !b.errors.is_empty() => {
                    Status::Unavailable
                }
                (Some(a), Some(b)) => {
                    if a.kind != b.kind {
                        differences.push(
                            if matches!(a.kind, PathKind::Dataset { .. })
                                && matches!(b.kind, PathKind::Dataset { .. })
                            {
                                "shape"
                            } else {
                                "object kind"
                            },
                        );
                    }
                    if a.datatype != b.datatype {
                        differences.push("datatype");
                    }
                    if a.attributes != b.attributes {
                        differences.push("attributes");
                    }
                    if differences.is_empty() {
                        Status::Same
                    } else {
                        Status::Different
                    }
                }
                (Some(_), None) => Status::OnlyA,
                (None, Some(_)) => Status::OnlyB,
                (None, None) => unreachable!(),
            };
            Row {
                path,
                a,
                b,
                status,
                differences,
            }
        })
        .collect()
}

#[derive(Default)]
pub(super) struct State {
    rows: Option<Vec<Row>>,
    job: Option<mpsc::Receiver<Result<Vec<Row>, String>>>,
    error: Option<String>,
    filter: Option<Status>,
    expanded: BTreeSet<String>,
    needs_redraw: bool,
}

impl State {
    pub fn invalidate(&mut self) {
        self.rows = None;
        self.job = None;
        self.error = None;
        self.filter = None;
        self.expanded.clear();
        self.needs_redraw = false;
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.needs_redraw)
    }

    pub(super) fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        match job.try_recv() {
            Ok(Ok(rows)) => {
                self.rows = Some(rows);
                self.job = None;
                self.needs_redraw = true;
            }
            Ok(Err(error)) => {
                self.error = Some(error);
                self.job = None;
                self.needs_redraw = true;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.error = Some("Comparison worker stopped unexpectedly".into());
                self.job = None;
                self.needs_redraw = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn start(&mut self, paths: &[PathBuf], options: OpenOptions, show_groups: bool) {
        let Some(a_path) = paths.first().cloned() else {
            return;
        };
        let Some(b_path) = paths.get(1).cloned() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = (|| {
                let a = File::open_with_options(&a_path, options).map_err(|e| e.to_string())?;
                let b = File::open_with_options(&b_path, options).map_err(|e| e.to_string())?;
                Ok(compare(&a, &b, show_groups))
            })();
            let _ = sender.send(result);
        });
        self.job = Some(receiver);
    }
}

pub(super) fn draw(lab: &mut PluginLab, ui: &Ui, context: PluginContext<'_>) -> Option<Selection> {
    let (Some(a), Some(b)) = (context.files.first(), context.files.get(1)) else {
        ui.spacing();
        ui.text_wrapped("Load files A and B from the Files menu to compare their paths.");
        return None;
    };
    lab.comparison.poll();
    if lab.comparison.rows.is_none()
        && lab.comparison.job.is_none()
        && lab.comparison.error.is_none()
    {
        lab.comparison.start(
            context.file_paths,
            OpenOptions {
                max_depth: context.settings.tree_max_depth,
                max_objects: context.settings.tree_max_objects,
            },
            context.settings.path_comparison_show_groups,
        );
    }
    if let Some(error) = &lab.comparison.error {
        ui.text_wrapped(format!("Comparison failed: {error}"));
        return None;
    }
    let Some(rows) = &lab.comparison.rows else {
        ui.text(format!("Comparing {} and {}...", a.name, b.name));
        return None;
    };
    let counts = [
        Status::Same,
        Status::Different,
        Status::OnlyA,
        Status::OnlyB,
        Status::Unavailable,
    ]
    .map(|status| rows.iter().filter(|row| row.status == status).count());
    let palette = &context.settings.palette;
    let available_width = ui.content_region_avail()[0];
    let mut line_width = 0.0;
    let filters = [
        (None, format!("All {}##filter_all", rows.len())),
        (
            Some(Status::Same),
            format!("Same {}##filter_same", counts[0]),
        ),
        (
            Some(Status::Different),
            format!("Different {}##filter_different", counts[1]),
        ),
        (
            Some(Status::OnlyA),
            format!("Only A {}##filter_only_a", counts[2]),
        ),
        (
            Some(Status::OnlyB),
            format!("Only B {}##filter_only_b", counts[3]),
        ),
        (
            Some(Status::Unavailable),
            format!("Unavailable {}##filter_unavailable", counts[4]),
        ),
    ];
    for (target, label) in filters {
        if target == Some(Status::Unavailable) && counts[4] == 0 {
            continue;
        }
        let width = ui.calc_text_size(label.split("##").next().unwrap_or(""))[0] + 20.0;
        if line_width > 0.0 && line_width + 8.0 + width <= available_width {
            ui.same_line_with_spacing(0.0, 8.0);
            line_width += 8.0 + width;
        } else {
            line_width = width;
        }
        let active = lab.comparison.filter == target;
        let color = target.map_or(palette.accent, |status| status_color(status, palette));
        let _button = ui.push_style_color(
            StyleColor::Button,
            if active {
                mix_color(palette.panel, color, 0.65)
            } else {
                mix_color(palette.panel, color, 0.28)
            },
        );
        let _border =
            ui.push_style_color(StyleColor::Border, mix_color(palette.border, color, 0.35));
        let _frame_border = ui.push_style_var(StyleVar::FrameBorderSize(1.0));
        let _hover = ui.push_style_color(
            StyleColor::ButtonHovered,
            mix_color(palette.panel, color, 0.46),
        );
        let _active = ui.push_style_color(
            StyleColor::ButtonActive,
            mix_color(palette.panel, color, 0.60),
        );
        if ui.button(label) {
            lab.comparison.filter = if active { None } else { target };
        }
    }
    let items = display_items(rows, lab.comparison.filter, &lab.comparison.expanded);
    let visible_paths = rows
        .iter()
        .filter(|row| {
            lab.comparison
                .filter
                .is_none_or(|status| row.status == status)
        })
        .count();
    {
        let _muted = ui.push_style_color(StyleColor::Text, palette.muted);
        ui.text_wrapped(format!("Showing {visible_paths} of {} paths. Click a path or chevron to reveal metadata on both sides; double-click a dataset path to inspect it.", rows.len()));
    }
    {
        let _muted = ui.push_style_color(StyleColor::Text, palette.muted);
        ui.text_wrapped("Matching compares paths, object kinds, shapes, datatypes, and attribute values. Dataset values are not compared.");
    }
    ui.spacing();
    let mut selected = None;
    let flags = TableFlags::SCROLL_Y | TableFlags::BORDERS_INNER_V | TableFlags::BORDERS_OUTER;
    let height = ui.content_region_avail()[1].max(1.0);
    let mut toggled_paths = Vec::new();
    if let Some(_table) =
        ui.begin_table_with_sizing("comparison_table", 3, flags, [0.0, height], 0.0)
    {
        ui.table_setup_column_stretch_weight("File A", TableColumnFlags::NONE, 1.0);
        ui.table_setup_column_fixed_width("Status", TableColumnFlags::NONE, 48.0);
        ui.table_setup_column_stretch_weight("File B", TableColumnFlags::NONE, 1.0);
        ui.table_setup_scroll_freeze(0, 1);
        ui.table_next_row();
        ui.table_set_cell_bg_color(mix_color(palette.panel, palette.accent, 0.35), 0);
        ui.table_set_cell_bg_color(palette.panel, 1);
        ui.table_set_cell_bg_color(mix_color(palette.panel, palette.heatmap_low, 0.35), 2);
        ui.table_next_column();
        ui.text(format!("A  ·  {}", a.name));
        if ui.is_item_hovered() {
            ui.set_item_tooltip(&a.name);
        }
        ui.table_next_column();
        center_status_cell(ui, ui.calc_text_size("A/B")[0]);
        ui.text("A/B");
        ui.table_next_column();
        ui.text(format!("B  ·  {}", b.name));
        if ui.is_item_hovered() {
            ui.set_item_tooltip(&b.name);
        }
        let mut clipper = ListClipper::new(items.len()).begin(ui);
        while clipper.step() {
            for index in clipper.display_start()..clipper.display_end() {
                let (row, kind, row_index) = items[index];
                ui.table_next_row();
                let tint = if row_index % 2 == 0 { 0.16 } else { 0.22 };
                let a_background = mix_color(palette.panel, palette.accent, tint);
                let b_background = mix_color(palette.panel, palette.heatmap_low, tint);
                ui.table_set_cell_bg_color(a_background, 0);
                ui.table_set_cell_bg_color(palette.background, 1);
                ui.table_set_cell_bg_color(b_background, 2);
                ui.table_next_column();
                match kind {
                    DisplayKind::Header => {
                        let expanded = lab.comparison.expanded.contains(&row.path);
                        let (toggled, next) = draw_entry_header(
                            ui,
                            row,
                            row.a.as_ref(),
                            0,
                            index,
                            expanded,
                            context.selection,
                            palette,
                            a_background,
                        );
                        if toggled {
                            toggled_paths.push(row.path.clone());
                        }
                        if let Some(next) = next {
                            selected = Some(next);
                        }
                        ui.table_next_column();
                        center_status_cell(ui, STATUS_ICON_SIZE[0]);
                        ui.invisible_button(format!("##status_{index}"), STATUS_ICON_SIZE);
                        draw_status_icon(
                            &ui.get_window_draw_list(),
                            row.status,
                            ui.item_rect_min(),
                            STATUS_ICON_SIZE,
                            status_color(row.status, palette),
                        );
                        if ui.is_item_hovered() {
                            let detail = if row.status == Status::Different {
                                format!("Different: {}", row.differences.join(", "))
                            } else if row.status == Status::Unavailable {
                                let errors = row
                                    .a
                                    .iter()
                                    .chain(row.b.iter())
                                    .flat_map(|summary| summary.errors.iter())
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                format!("Metadata unavailable\n{errors}")
                            } else {
                                row.status.label().into()
                            };
                            ui.set_item_tooltip(detail);
                        }
                        ui.table_next_column();
                        let (toggled, next) = draw_entry_header(
                            ui,
                            row,
                            row.b.as_ref(),
                            1,
                            index,
                            expanded,
                            context.selection,
                            palette,
                            b_background,
                        );
                        if toggled {
                            toggled_paths.push(row.path.clone());
                        }
                        if let Some(next) = next {
                            selected = Some(next);
                        }
                    }
                    DisplayKind::Metadata | DisplayKind::Attributes => {
                        let attributes = kind == DisplayKind::Attributes;
                        draw_entry_detail(ui, row.a.as_ref(), attributes, palette);
                        ui.table_next_column();
                        center_status_cell(ui, STATUS_ICON_SIZE[0]);
                        ui.dummy(STATUS_ICON_SIZE);
                        ui.table_next_column();
                        draw_entry_detail(ui, row.b.as_ref(), attributes, palette);
                    }
                }
            }
        }
    }
    toggled_paths.sort();
    toggled_paths.dedup();
    for path in toggled_paths {
        if !lab.comparison.expanded.insert(path.clone()) {
            lab.comparison.expanded.remove(&path);
        }
        lab.comparison.needs_redraw = true;
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    fn demo_files() -> ([PathBuf; 2], [File; 2]) {
        let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/path_comparison");
        let paths = ["plugin_demo_a.hdf5", "plugin_demo_b.hdf5"].map(|name| examples.join(name));
        let files = paths.each_ref().map(|path| File::open(path).unwrap());
        (paths, files)
    }

    #[test]
    fn compares_demo_shapes_datatypes_attributes_and_paths() {
        let (_, [a, b]) = demo_files();
        let rows = compare(&a, &b, true);
        let row = |path: &str| rows.iter().find(|row| row.path == path).unwrap();
        assert_eq!(row("/measurements/shared").status, Status::Same);
        assert_eq!(row("/measurements/value_only").status, Status::Same);
        assert_eq!(row("/measurements/changed_shape").differences, ["shape"]);
        assert_eq!(
            row("/measurements/changed_datatype").differences,
            ["datatype"]
        );
        assert_eq!(
            row("/measurements/changed_attribute").differences,
            ["attributes"]
        );
        assert_eq!(
            row("/measurements/missing_attribute").differences,
            ["attributes"]
        );
        assert_eq!(row("/measurements").differences, ["attributes"]);
        assert_eq!(
            row("/structure_variant").differences,
            ["object kind", "datatype", "attributes"]
        );
        assert_eq!(row("/measurements/only_a").status, Status::OnlyA);
        assert_eq!(row("/measurements/only_b").status, Status::OnlyB);
        assert_eq!(row("/coordinates/latitude").status, Status::Same);
    }

    #[test]
    fn filters_paths_and_expands_both_columns_together() {
        let (_, [a, b]) = demo_files();
        let rows = compare(&a, &b, true);
        let mut expanded = BTreeSet::new();
        let different_count = rows
            .iter()
            .filter(|row| row.status == Status::Different)
            .count();
        assert_eq!(
            display_items(&rows, Some(Status::Different), &expanded).len(),
            different_count
        );
        expanded.insert("/measurements/changed_shape".into());
        let items = display_items(&rows, Some(Status::Different), &expanded);
        let kinds: Vec<_> = items
            .iter()
            .filter(|(row, _, _)| row.path == "/measurements/changed_shape")
            .map(|(_, kind, _)| *kind)
            .collect();
        assert_eq!(
            kinds,
            [
                DisplayKind::Header,
                DisplayKind::Metadata,
                DisplayKind::Attributes
            ]
        );
        assert!(items
            .iter()
            .all(|(row, _, _)| row.status == Status::Different));
        assert_eq!(
            display_items(&rows, Some(Status::OnlyA), &expanded).len(),
            rows.iter()
                .filter(|row| row.status == Status::OnlyA)
                .count()
        );
    }

    #[test]
    fn side_by_side_table_renders_with_one_scroll_area() {
        let (paths, files) = demo_files();
        let settings = Settings::load().unwrap();
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([900.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut lab = PluginLab::default();
        lab.comparison.rows = Some(compare(&files[0], &files[1], true));
        lab.comparison
            .expanded
            .insert("/measurements/changed_shape".into());
        let ui = context.frame();
        ui.window("Comparison")
            .size([850.0, 550.0], Condition::Always)
            .build(|| {
                assert!(draw(
                    &mut lab,
                    ui,
                    PluginContext {
                        files: &files,
                        file_paths: &paths,
                        settings: &settings,
                        selection: None,
                    }
                )
                .is_none());
            });
        context.end_frame();
    }
}
