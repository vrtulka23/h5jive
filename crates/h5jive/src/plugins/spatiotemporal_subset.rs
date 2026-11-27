use super::*;
use crate::app::coastlines;
use h5jive_data::{
    export_subset, inspect_subset_product_with_rules, preview_subset_coverage, CoveragePreview,
    MosaicMethod, SubsetBounds, SubsetMode, SubsetRequest, SubsetVariable,
};
use std::{path::PathBuf, sync::Arc};

pub(super) struct State {
    source: usize,
    product_selection: Option<(usize, u64, ProductRange, Arc<Vec<PathBuf>>)>,
    available: Option<(usize, u64, ProductAvailability)>,
    from_date: String,
    through_date: String,
    from_orbit: String,
    through_orbit: String,
    west: String,
    south: String,
    east: String,
    north: String,
    variables: Vec<SubsetVariable>,
    enabled: Vec<bool>,
    inspected: Option<PathBuf>,
    inspection: BackgroundJob<PathBuf, Vec<SubsetVariable>>,
    coverage: Option<CoveragePreview>,
    coverage_job: BackgroundJob<(), CoveragePreview>,
    coverage_error: Option<String>,
    mode: SubsetMode,
    series_group: String,
    series_dimension: String,
    mosaic_method: MosaicMethod,
    resolution: String,
    pub(super) output: String,
    export_job: BackgroundJob<(), String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            source: 0,
            product_selection: None,
            available: None,
            from_date: String::new(),
            through_date: String::new(),
            from_orbit: String::new(),
            through_orbit: String::new(),
            west: "-180".into(),
            south: "-90".into(),
            east: "180".into(),
            north: "90".into(),
            variables: Vec::new(),
            enabled: Vec::new(),
            inspected: None,
            inspection: BackgroundJob::default(),
            coverage: None,
            coverage_job: BackgroundJob::default(),
            coverage_error: None,
            mode: SubsetMode::Mosaic,
            series_group: String::new(),
            series_dimension: "product".into(),
            mosaic_method: MosaicMethod::Mean,
            resolution: "1".into(),
            output: "subset.nc".into(),
            export_job: BackgroundJob::default(),
        }
    }
}
impl State {
    pub fn invalidate(&mut self) {
        self.product_selection = None;
        self.available = None;
        self.inspected = None;
        self.inspection.cancel();
        self.variables.clear();
        self.enabled.clear();
        self.coverage = None;
        self.coverage_job.cancel();
        self.coverage_error = None;
    }
    pub fn poll(&mut self, notices: &mut NoticeCenter) -> bool {
        let mut changed = false;
        if let Some((_, result)) = self.inspection.poll("Product inspection stopped") {
            match result {
                Ok(vars) => {
                    self.enabled = vec![false; vars.len()];
                    if !vars.is_empty() {
                        if vars[0].shape.len() == 4 {
                            self.mode = SubsetMode::Native;
                        }
                    }
                    self.variables = vars;
                    changed = true;
                }
                Err(e) => {
                    notices.error(e);
                    changed = true;
                }
            }
        }
        if let Some((_, result)) = self.coverage_job.poll("Coverage preview stopped") {
            match result {
                Ok(preview) => {
                    self.coverage = Some(preview);
                    self.coverage_error = None;
                    changed = true;
                }
                Err(e) => {
                    notices.error(format!("Coverage preview stopped: {e}"));
                    self.coverage_error = Some(e);
                    changed = true;
                }
            }
        }
        if let Some((_, result)) = self.export_job.poll("Export worker stopped") {
            match result {
                Ok(message) => notices.success(message),
                Err(error) => notices.error(format!("Could not export: {error}")),
            }
            changed = true;
        }
        changed
            || self.inspection.is_busy()
            || self.coverage_job.is_busy()
            || self.export_job.is_busy()
    }

    pub fn is_busy(&self) -> bool {
        self.inspection.is_busy() || self.coverage_job.is_busy() || self.export_job.is_busy()
    }

    pub fn active_task_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.inspection.is_busy() {
            labels.push("product inspection");
        }
        if self.coverage_job.is_busy() {
            labels.push("coverage preview");
        }
        if self.export_job.is_busy() {
            labels.push("subset export");
        }
        labels
    }
    fn bounds(&self) -> Result<SubsetBounds, String> {
        let parse = |s: &str| {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("Invalid geographic bound: {s}"))
        };
        let bounds = SubsetBounds {
            west: parse(&self.west)?,
            south: parse(&self.south)?,
            east: parse(&self.east)?,
            north: parse(&self.north)?,
        };
        if bounds.valid() {
            Ok(bounds)
        } else {
            Err("Use latitudes from -90 to 90 and longitudes from -180 to 180; south must be below north".into())
        }
    }
}
fn selected_products(context: &PluginContext<'_>, state: &mut State) -> Arc<Vec<PathBuf>> {
    let Some(source) = context.settings.dataset_sources.get(state.source) else {
        return Arc::new(Vec::new());
    };
    let range = ProductRange {
        from_date: state.from_date.clone(),
        through_date: state.through_date.clone(),
        from_orbit: state.from_orbit.clone(),
        through_orbit: state.through_orbit.clone(),
    };
    let revision = context.browser.products_revision();
    if let Some((source, old_revision, old_range, paths)) = &state.product_selection {
        if *source == state.source && *old_revision == revision && *old_range == range {
            return Arc::clone(paths);
        }
    }
    state.coverage = None;
    state.coverage_job.cancel();
    state.coverage_error = None;
    let paths: Arc<Vec<PathBuf>> = Arc::new(
        range
            .select(context.browser.products(state.source))
            .into_iter()
            .map(|product| source.root.join(&product.relative_path))
            .collect(),
    );
    if state.inspected.as_ref() != paths.first() {
        state.inspected = None;
        state.inspection.cancel();
        state.variables.clear();
        state.enabled.clear();
    }
    state.product_selection = Some((state.source, revision, range, Arc::clone(&paths)));
    paths
}
fn selected_variables(state: &State) -> Vec<SubsetVariable> {
    state
        .variables
        .iter()
        .zip(&state.enabled)
        .filter_map(|(v, &enabled)| enabled.then_some(v.clone()))
        .collect()
}
fn editable_bounds(ui: &Ui, state: &mut State, tutorial_tips: bool) {
    ui.text("Spatial bounds (degrees)");
    if let Some(_table) = ui.begin_table("subset_bounds", 4) {
        for label in ["W", "S", "E", "N"] {
            ui.table_setup_column_stretch_weight(label, TableColumnFlags::NONE, 1.0);
        }
        ui.table_next_row();
        for (short, label, field) in [
            ("W", "West longitude", &mut state.west),
            ("S", "South latitude", &mut state.south),
            ("E", "East longitude", &mut state.east),
            ("N", "North latitude", &mut state.north),
        ] {
            ui.table_next_column();
            ui.text(short);
            ui.same_line();
            ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
            ui.input_text(format!("##subset_{short}"), field).build();
            item_tip(
                ui,
                tutorial_tips,
                &format!("{label} in degrees."),
                &format!("Set the {label} of the geographic area to export."),
            );
        }
    }
}
fn range_inputs(
    ui: &Ui,
    table_id: &str,
    from_id: &str,
    through_id: &str,
    from: &mut String,
    through: &mut String,
) {
    if let Some(_table) = ui.begin_table(table_id, 2) {
        ui.table_setup_column_stretch_weight("From", TableColumnFlags::NONE, 1.0);
        ui.table_setup_column_stretch_weight("Through", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        ui.text("From");
        ui.same_line();
        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
        ui.input_text(from_id, from).build();
        ui.table_next_column();
        ui.text("Through");
        ui.same_line();
        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
        ui.input_text(through_id, through).build();
    }
}
fn draw_world_coverage(ui: &Ui, coverage: &CoveragePreview, settings: &Settings, width: f32) {
    ui.text("World coverage");
    ui.set_cursor_pos_x(ui.cursor_pos()[0] + (ui.content_region_avail()[0] - width).max(0.0) * 0.5);
    let height = width * 0.5;
    let min = ui.cursor_screen_pos();
    let max = [min[0] + width, min[1] + height];
    ui.dummy([width, height]);
    let draw = ui.get_window_draw_list();
    draw.add_rect(min, max, settings.palette.panel)
        .filled(true)
        .build();
    let grid = muted_geo_color(settings.palette.border, 0.42);
    let shore = muted_geo_color(settings.palette.muted, 0.7);
    let projection = MapViewport::default();
    for lon in (-180..=180).step_by(30) {
        let x = projection.project(min, [width, height], lon as f32, 0.0)[0];
        draw.add_line([x, min[1]], [x, max[1]], grid).build();
    }
    for lat in (-90..=90).step_by(30) {
        let y = projection.project(min, [width, height], 0.0, lat as f32)[1];
        draw.add_line([min[0], y], [max[0], y], grid).build();
    }
    for path in coastlines() {
        for segment in path.windows(2) {
            if (segment[0][0] - segment[1][0]).abs() <= 180.0 {
                draw.add_line(
                    projection.project(min, [width, height], segment[0][0], segment[0][1]),
                    projection.project(min, [width, height], segment[1][0], segment[1][1]),
                    shore,
                )
                .build();
            }
        }
    }
    draw.add_rect(min, max, settings.palette.border).build();
    for lon in [-180, -90, 0, 90] {
        let x = projection.project(min, [width, height], lon as f32, 0.0)[0];
        draw.add_text(
            [x + 3.0, max[1] - ui.text_line_height() - 2.0],
            settings.palette.muted,
            format!("{lon}"),
        );
    }
    for lat in [-60, 0, 60] {
        let y = projection.project(min, [width, height], 0.0, lat as f32)[1];
        draw.add_text(
            [min[0] + 4.0, y - ui.text_line_height() - 2.0],
            settings.palette.muted,
            format!("{lat}"),
        );
    }
    let max_count = coverage.counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    for cell in coverage_cells(coverage.width, coverage.height, &coverage.counts) {
        let point = projection.project(min, [width, height], cell.longitude, cell.latitude);
        let alpha = 0.35 + 0.65 * (cell.count as f32 / max_count).sqrt();
        let c = settings.palette.accent;
        draw.add_circle(
            point,
            1.4 + (cell.count as f32).sqrt().min(2.0),
            [c[0], c[1], c[2], alpha],
        )
        .filled(true)
        .build();
    }
}

fn muted_geo_color(color: [f32; 4], opacity: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * opacity]
}

fn draw_globe_coverage(
    ui: &Ui,
    coverage: &CoveragePreview,
    settings: &Settings,
    label: &str,
    yaw: f32,
    diameter: f32,
) {
    ui.text(label);
    let min = ui.cursor_screen_pos();
    ui.dummy([diameter, diameter]);
    let center = [min[0] + diameter * 0.5, min[1] + diameter * 0.5];
    let radius = diameter * 0.44;
    let draw = ui.get_window_draw_list();
    draw.add_circle(center, radius, settings.palette.panel)
        .filled(true)
        .build();
    draw.add_circle(center, radius, settings.palette.border)
        .build();
    let projection = GlobeProjection {
        center,
        radius,
        yaw,
        pitch: 0.0,
    };
    let grid = muted_geo_color(settings.palette.border, 0.42);
    let shore = muted_geo_color(settings.palette.muted, 0.7);
    for lon in (-150..=180).step_by(30) {
        let mut last = None;
        for lat in (-90..=90).step_by(3) {
            let point = projection.project(lon as f32, lat as f32);
            if point.depth >= 0.0 {
                if let Some(previous) = last {
                    draw.add_line(previous, point.point, grid).build();
                }
                last = Some(point.point);
            } else {
                last = None;
            }
        }
    }
    for lat in (-60..=60).step_by(30) {
        let mut last = None;
        for lon in (-180..=180).step_by(3) {
            let point = projection.project(lon as f32, lat as f32);
            if point.depth >= 0.0 {
                if let Some(previous) = last {
                    draw.add_line(previous, point.point, grid).build();
                }
                last = Some(point.point);
            } else {
                last = None;
            }
        }
    }
    for path in coastlines() {
        for segment in path.windows(2) {
            let a = projection.project(segment[0][0], segment[0][1]);
            let b = projection.project(segment[1][0], segment[1][1]);
            if a.depth >= 0.0 && b.depth >= 0.0 {
                draw.add_line(a.point, b.point, shore).build();
            }
        }
    }
    draw.add_text(
        [center[0] - 5.0, center[1] - radius + 3.0],
        settings.palette.muted,
        "N",
    );
    draw.add_text(
        [
            center[0] - 4.0,
            center[1] + radius - ui.text_line_height() - 2.0,
        ],
        settings.palette.muted,
        "S",
    );
    let max_count = coverage.counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    for cell in coverage_cells(coverage.width, coverage.height, &coverage.counts) {
        let point = projection.project(cell.longitude, cell.latitude);
        if point.depth <= 0.0 {
            continue;
        }
        let alpha = 0.35 + 0.65 * (cell.count as f32 / max_count).sqrt();
        let c = settings.palette.accent;
        draw.add_circle(
            point.point,
            1.2 + (cell.count as f32).sqrt().min(2.0),
            [c[0], c[1], c[2], alpha],
        )
        .filled(true)
        .build();
    }
}

struct CoveragePopupLayout {
    size: [f32; 2],
    map_width: f32,
    globe_diameter: f32,
}

fn coverage_popup_layout(display: [f32; 2], padding: [f32; 2]) -> CoveragePopupLayout {
    let max_height = (display[1] - 40.0).max(200.0);
    let max_width = (display[0] - 40.0).max(200.0);
    let scale = ((max_height - 120.0) / 400.0)
        .min((max_width - 2.0 * padding[0] - 12.0) / 420.0)
        .clamp(0.25, 1.0);
    let map_width = 420.0 * scale;
    let globe_diameter = 190.0 * scale;
    CoveragePopupLayout {
        size: [
            map_width + 2.0 * padding[0] + 12.0,
            map_width * 0.5 + globe_diameter + 120.0,
        ],
        map_width,
        globe_diameter,
    }
}

fn draw_coverage_popup(ui: &Ui, state: &State, settings: &Settings, layout: &CoveragePopupLayout) {
    if let Some(coverage) = &state.coverage {
        ui.text(format!(
            "{} products, {} sampled points",
            coverage.products, coverage.sampled_points
        ));
        draw_world_coverage(ui, coverage, settings, layout.map_width);
        let table_width = layout.globe_diameter * 2.0 + 24.0;
        ui.set_cursor_pos_x(
            ui.cursor_pos()[0] + (ui.content_region_avail()[0] - table_width).max(0.0) * 0.5,
        );
        if let Some(_table) = ui.begin_table_with_sizing(
            "subset_coverage_hemispheres",
            2,
            TableFlags::NONE,
            [table_width, 0.0],
            0.0,
        ) {
            for label in ["0 deg center", "180 deg center"] {
                ui.table_setup_column_fixed_width(
                    label,
                    TableColumnFlags::NONE,
                    layout.globe_diameter + 8.0,
                );
            }
            ui.table_next_row();
            ui.table_next_column();
            draw_globe_coverage(
                ui,
                coverage,
                settings,
                "0 deg center",
                0.0,
                layout.globe_diameter,
            );
            ui.table_next_column();
            draw_globe_coverage(
                ui,
                coverage,
                settings,
                "180 deg center",
                std::f32::consts::PI,
                layout.globe_diameter,
            );
        }
    } else if let Some(error) = &state.coverage_error {
        ui.text_wrapped(error);
    } else {
        ui.text("No coverage preview is available yet.");
    }
}
pub(super) fn draw_icon(draw: &DrawListMut<'_>, min: [f32; 2], max: [f32; 2], color: [f32; 4]) {
    let x = (min[0] + max[0]) * 0.5;
    let y = (min[1] + max[1]) * 0.5;
    draw.add_circle([x, y], 11.0, color).thickness(1.5).build();
    draw.add_line([x - 10.0, y], [x + 10.0, y], color).build();
    draw.add_line([x, y - 10.0], [x, y + 10.0], color).build();
    draw.add_rect([x + 2.0, y - 7.0], [x + 9.0, y], color)
        .filled(true)
        .build();
}
pub(super) fn draw(lab: &mut PluginLab, ui: &Ui, context: PluginContext<'_>) -> Option<Selection> {
    const COVERAGE_POPUP: &str = "Coverage preview###subset_coverage_preview";
    let state = &mut lab.spatiotemporal_subset;
    if context.settings.dataset_sources.is_empty() {
        ui.text_wrapped(
            "Register a dataset source in Dataset Browser settings to use this plugin.",
        );
        return None;
    }
    let labels = context
        .settings
        .dataset_sources
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>();
    state.source = state.source.min(labels.len() - 1);
    let mut show_preview = false;
    let products = if let Some(_table) = ui.begin_table("subset_inputs", 2) {
        ui.table_setup_column_stretch_weight("Source and ranges", TableColumnFlags::NONE, 1.0);
        ui.table_setup_column_stretch_weight("Variables", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        let left_top = ui.cursor_screen_pos()[1];
        ui.text("Dataset source");
        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
        if ui.combo_simple_string("##subset_source", &mut state.source, &labels) {
            state.invalidate();
        }
        item_tip(ui,context.settings.tutorial_tips,"Use products already indexed by Dataset Browser.","Choose a registered Dataset Browser source. Product discovery and coordinates are shared with the Browser.");
        let revision = context.browser.products_revision();
        if state
            .available
            .as_ref()
            .is_none_or(|(source, old_revision, _)| {
                *source != state.source || *old_revision != revision
            })
        {
            state.available = Some((
                state.source,
                revision,
                ProductAvailability::from_products(context.browser.products(state.source)),
            ));
        }
        let available = &state.available.as_ref().unwrap().2;
        ui.text("Dates (YYYY-MM-DD)");
        ui.same_line();
        if let Some((first, last)) = &available.dates {
            ui.text_disabled(format!("{first} to {last}"));
        } else {
            ui.text_disabled("No indexed dates");
        }
        range_inputs(
            ui,
            "subset_date_inputs",
            "##subset_date_from",
            "##subset_date_through",
            &mut state.from_date,
            &mut state.through_date,
        );
        ui.text("Orbits");
        ui.same_line();
        if let Some((first, last)) = &available.orbits {
            ui.text_disabled(format!("{first} to {last}"));
        } else {
            ui.text_disabled("No indexed orbits");
        }
        range_inputs(
            ui,
            "subset_orbit_inputs",
            "##subset_orbit_from",
            "##subset_orbit_through",
            &mut state.from_orbit,
            &mut state.through_orbit,
        );
        let products = selected_products(&context, state);
        ui.text(format!("{} indexed products selected", products.len()));
        if let Some(first) = products.first() {
            if state.inspected.as_ref() != Some(first) && !state.inspection.is_busy() {
                state.inspected = Some(first.clone());
                state.variables.clear();
                state.enabled.clear();
                state.coverage = None;
                state.coverage_error = None;
                let path = first.clone();
                let rules = context.settings.dataset_rules.clone();
                state.inspection.start(path.clone(), move |_| {
                    inspect_subset_product_with_rules(&path, &rules)
                });
            }
        }
        editable_bounds(ui, state, context.settings.tutorial_tips);
        show_preview = ui.button("Preview selected coverage");
        item_tip(
            ui,
            context.settings.tutorial_tips,
            "Open the coverage preview.",
            "Open a world map and two globe hemispheres for the selected products.",
        );
        let left_height = ui.cursor_screen_pos()[1] - left_top;
        ui.table_next_column();
        let right_top = ui.cursor_screen_pos()[1];
        ui.text("Variables");
        let header_height = ui.cursor_screen_pos()[1] - right_top;
        ui.child_window("subset_variable_panel")
            .size([0.0, (left_height - header_height).max(230.0)])
            .border(true)
            .flags(WindowFlags::HORIZONTAL_SCROLLBAR)
            .build(ui, || {
                if state.inspection.is_busy() {
                    ui.text("Inspecting representative product...");
                } else if state.variables.is_empty() {
                    ui.text_wrapped("No supported geographic numeric fields found. The source needs numeric fields with identifiable latitude and longitude coordinates.");
                } else {
                    let mut clipper = ListClipper::new(state.variables.len()).begin(ui);
                    while clipper.step() {
                        for i in clipper.display_start()..clipper.display_end() {
                            let variable = &state.variables[i];
                            let label = format!(
                                "{}  {:?}##subset_variable_{i}",
                                variable.path, variable.shape
                            );
                            if ui.checkbox(label, &mut state.enabled[i]) {
                                state.coverage = None;
                                state.coverage_job.cancel();
                                state.coverage_error = None;
                            }
                            item_tip(ui,context.settings.tutorial_tips,"Include this variable in the export.","Select fields sharing the same spatial coordinates and shape. The export checks each selected product for compatible fields.");
                        }
                    }
                }
            });
        products
    } else {
        selected_products(&context, state)
    };
    if show_preview {
        ui.open_popup(COVERAGE_POPUP);
        state.coverage_error = None;
        if state.coverage.is_none() && !state.coverage_job.is_busy() {
            if products.is_empty() {
                state.coverage_error = Some("Select at least one indexed product".into());
            } else if let Some(choice) = selected_variables(state).first().cloned() {
                let paths = products.as_ref().clone();
                let max_points = context.settings.spatiotemporal_subset_preview_max_points;
                state.coverage_job.start((), move |_| {
                    preview_subset_coverage(&paths, &choice, max_points)
                });
            } else {
                state.coverage_error = Some("Select at least one variable".into());
            }
        }
    }
    let mut popup_open = true;
    if let Some(_popup) = ui
        .begin_modal_popup_config(COVERAGE_POPUP)
        .opened(&mut popup_open)
        .flags(
            WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE | WindowFlags::NO_RESIZE,
        )
        .begin()
    {
        let display = ui.io().display_size();
        let layout = coverage_popup_layout(display, ui.clone_style().window_padding());
        ui.set_window_size(layout.size);
        ui.set_window_pos([
            (display[0] - layout.size[0]) * 0.5,
            (display[1] - layout.size[1]) * 0.5,
        ]);
        draw_coverage_popup(ui, state, context.settings, &layout);
    }
    ui.separator();
    if let Some(_table) = ui.begin_table("subset_export_panels", 2) {
        ui.table_setup_column_fixed_width("Export view", TableColumnFlags::NONE, 180.0);
        ui.table_setup_column_stretch_weight("Output", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        ui.text("Export view");
        dataset_manipulation::segmented_control(
            ui,
            "subset_export_mode",
            &mut state.mode,
            &[
                (
                    SubsetMode::Native,
                    "Native products",
                    dataset_manipulation::SegmentIcon::Native,
                    "Keep cropped products separate with their coordinates, footprints, scan direction, time samples, and all channels.",
                ),
                (
                    SubsetMode::TimeSeries,
                    "Combined time series",
                    dataset_manipulation::SegmentIcon::TimeSeries,
                    "Combine grids or swaths along one time axis. Swaths keep per-sample coordinates; products with no time coordinate use a named index dimension.",
                ),
                (
                    SubsetMode::Mosaic,
                    "Gridded mosaic",
                    dataset_manipulation::SegmentIcon::Mosaic,
                    "Average overlapping samples separately for every channel and write sample counts.",
                ),
            ],
            dataset_manipulation::SegmentLayout::Vertical,
            Some(180.0),
            context.settings.tutorial_tips,
            context.settings.palette.accent,
            context.settings.palette.border,
        );
        ui.table_next_column();
        ui.text("New NetCDF file");
        if state.mode == SubsetMode::Mosaic {
            ui.input_text("Grid resolution (degrees)", &mut state.resolution)
                .build();
            let methods = [
                MosaicMethod::Mean,
                MosaicMethod::Newest,
                MosaicMethod::Oldest,
                MosaicMethod::Minimum,
                MosaicMethod::Maximum,
            ];
            let mut selected = methods
                .iter()
                .position(|&method| method == state.mosaic_method)
                .unwrap_or(0);
            if ui.combo_simple_string(
                "Mosaic method",
                &mut selected,
                &["Mean", "Newest", "Oldest", "Minimum", "Maximum"],
            ) {
                state.mosaic_method = methods[selected];
            }
            item_tip(ui, context.settings.tutorial_tips,
                "Choose how overlapping valid samples become one cell value.",
                "Mean averages all valid samples. Newest and Oldest use indexed product order and CF time within a product. Minimum and Maximum choose value extremes. Counts always show all valid samples.");
        }
        if state.mode == SubsetMode::TimeSeries {
            ui.input_text("Group name (optional)", &mut state.series_group)
                .build();
            item_tip(
                ui,
                context.settings.tutorial_tips,
                "Place the combined data in a named group.",
                "Leave this empty to write at the NetCDF root.",
            );
            if !selected_variables(state)
                .first()
                .is_some_and(|variable| variable.time.is_some())
            {
                ui.input_text("First dimension name", &mut state.series_dimension)
                    .build();
                item_tip(ui,context.settings.tutorial_tips,"Name the leading index dimension.","The source has no CF time coordinate, so this dimension contains sample indexes, not timestamps.");
            } else {
                let selected = selected_variables(state);
                let name = selected
                    .first()
                    .and_then(|variable| variable.time.as_deref())
                    .unwrap_or("time");
                ui.text_disabled(format!(
                    "Leading dimension: {} (from source)",
                    name.rsplit('/').next().unwrap_or(name)
                ));
            }
        }
        if file_path_field(
            ui,
            "subset_new_file",
            &mut state.output,
            ui.content_region_avail()[0],
        ) {
            *context.picker_request = Some((FilePickerTarget::SubsetOutput, state.output.clone()));
        }
        item_tip(ui,context.settings.tutorial_tips,"Existing output files are protected.","Enter an output .nc filename or absolute path. The exporter writes a new NetCDF-4 file and preserves existing files.");
        ui.spacing();
        let exporting = state.export_job.is_busy();
        let export_clicked = {
            let _disabled = ui.begin_disabled_with_cond(exporting);
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
            ui.button(if exporting {
                "Exporting NetCDF-4..."
            } else {
                "Export NetCDF-4"
            })
        };
        item_tip(
            ui,
            context.settings.tutorial_tips,
            "Export the selected products and variables to a new NetCDF-4 file.",
            "The output file is created at the path above. Existing files are protected.",
        );
        if export_clicked {
            let result: Result<SubsetRequest, String> = (|| {
                let parse_date = |text: &str| -> Result<Option<String>, String> {
                    if text.trim().is_empty() {
                        return Ok(None);
                    }
                    let digits = date_query(text).ok_or("Use dates in YYYY-MM-DD form")?;
                    if digits.len() != 8 {
                        return Err("Use dates in YYYY-MM-DD form".into());
                    }
                    Ok(Some(digits))
                };
                let from = parse_date(&state.from_date)?;
                let through = parse_date(&state.through_date)?;
                if from
                    .as_ref()
                    .zip(through.as_ref())
                    .is_some_and(|(a, b)| a > b)
                {
                    return Err("Start date must be before end date".into());
                }
                let bounds = state.bounds()?;
                let variables = selected_variables(state);
                if products.is_empty() {
                    return Err("Select at least one indexed product".into());
                }
                if variables.is_empty() {
                    return Err("Select at least one variable from the Variables list".into());
                }
                let output = PathBuf::from(state.output.trim());
                if output.as_os_str().is_empty()
                    || output.extension().and_then(|s| s.to_str()) != Some("nc")
                {
                    return Err("Choose a new .nc output file".into());
                }
                let resolution = state
                    .resolution
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| "Invalid grid resolution")?;
                Ok(SubsetRequest {
                    products: (*products).clone(),
                    variables,
                    bounds,
                    from_date: state.from_date.clone(),
                    through_date: state.through_date.clone(),
                    mode: state.mode,
                    series_group: state.series_group.trim().to_owned(),
                    series_dimension: state.series_dimension.trim().to_owned(),
                    mosaic_method: state.mosaic_method,
                    resolution,
                    output,
                    geolocation_rules: context.settings.dataset_rules.clone(),
                })
            })();
            match result {
                Ok(request) => {
                    state.export_job.start((), move |_| export_subset(&request));
                }
                Err(e) => context.notices.error(format!("Could not export: {e}")),
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    #[test]
    fn coverage_popup_content_fits_without_scrolling() {
        let settings = Settings::load().unwrap();
        for display in [[1100.0, 800.0], [800.0, 600.0], [500.0, 450.0]] {
            let mut context = Context::create();
            context.set_ini_filename(None::<String>).unwrap();
            context.io_mut().set_display_size(display);
            context.io_mut().set_delta_time(1.0 / 60.0);
            context
                .font_atlas()
                .try_claim_legacy_renderer()
                .unwrap()
                .build();
            let mut state = State::default();
            let mut counts = vec![0_u16; 360 * 180];
            counts[90 * 360 + 180] = 2;
            state.coverage = Some(CoveragePreview {
                width: 360,
                height: 180,
                counts,
                products: 2,
                sampled_points: 50,
            });
            for frame in 0..3 {
                let ui = context.frame();
                ui.window("Subset")
                    .size(display, Condition::Always)
                    .build(|| {
                        if frame == 0 {
                            ui.open_popup("Coverage preview###subset_coverage_preview");
                        }
                        if let Some(_popup) = ui
                            .begin_modal_popup_config("Coverage preview###subset_coverage_preview")
                            .flags(WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE)
                            .begin()
                        {
                            let layout =
                                coverage_popup_layout(display, ui.clone_style().window_padding());
                            ui.set_window_size(layout.size);
                            ui.set_window_pos([
                                (display[0] - layout.size[0]) * 0.5,
                                (display[1] - layout.size[1]) * 0.5,
                            ]);
                            draw_coverage_popup(ui, &state, &settings, &layout);
                            if frame > 0 {
                                assert!(
                                    ui.content_region_avail()[1] >= 0.0,
                                    "content exceeds popup at {display:?}"
                                );
                                assert_eq!(
                                    ui.scroll_max_x(),
                                    0.0,
                                    "horizontal scroll at {display:?}"
                                );
                                assert_eq!(
                                    ui.scroll_max_y(),
                                    0.0,
                                    "vertical scroll at {display:?}"
                                );
                            }
                        }
                    });
                context.end_frame();
            }
        }
    }

    #[test]
    fn controls_and_coverage_render_without_loaded_inspector_files() {
        let settings = Settings::load().unwrap();
        let browser = DatasetBrowser::new(&[]);
        let mut picker_request = None;
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([1100.0, 800.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut lab = PluginLab::default();
        lab.spatiotemporal_subset.mode = SubsetMode::TimeSeries;
        let mut counts = vec![0_u16; 360 * 180];
        counts[90 * 360 + 180] = 2;
        lab.spatiotemporal_subset.coverage = Some(CoveragePreview {
            width: 360,
            height: 180,
            counts,
            products: 2,
            sampled_points: 50,
        });
        let mut files = [];
        let paths = [];
        let ui = context.frame();
        ui.window("Subset")
            .size([1000.0, 750.0], Condition::Always)
            .build(|| {
                ui.open_popup("Coverage preview###subset_coverage_preview");
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
                assert!(ui.is_popup_open("Coverage preview###subset_coverage_preview"));
            });
        context.end_frame();
    }
}
