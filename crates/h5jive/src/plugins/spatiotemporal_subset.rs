use super::*;
use h5jive_data::{
    export_subset, inspect_subset_product_with_rules, preview_subset_coverage, CoveragePreview,
    SubsetBounds, SubsetMode, SubsetRequest, SubsetVariable,
};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
};

pub(super) struct State {
    source: usize,
    product_selection: Option<(usize, u64, ProductRange, Arc<Vec<PathBuf>>)>,
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
    inspection: Option<mpsc::Receiver<Result<Vec<SubsetVariable>, String>>>,
    coverage: Option<CoveragePreview>,
    coverage_job: Option<mpsc::Receiver<Result<CoveragePreview, String>>>,
    preview_globe: bool,
    globe_yaw: f64,
    globe_pitch: f64,
    drag_start: Option<[f32; 2]>,
    mode: SubsetMode,
    resolution: String,
    pub(super) output: String,
    export_job: Option<mpsc::Receiver<Result<String, String>>>,
    status: Option<Result<String, String>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            source: 0,
            product_selection: None,
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
            inspection: None,
            coverage: None,
            coverage_job: None,
            preview_globe: false,
            globe_yaw: 0.0,
            globe_pitch: 0.0,
            drag_start: None,
            mode: SubsetMode::Mosaic,
            resolution: "1".into(),
            output: "subset.nc".into(),
            export_job: None,
            status: None,
        }
    }
}
impl State {
    pub fn invalidate(&mut self) {
        self.product_selection = None;
        self.inspected = None;
        self.inspection = None;
        self.variables.clear();
        self.enabled.clear();
        self.coverage = None;
        self.coverage_job = None;
    }
    pub fn is_busy(&mut self) -> bool {
        let mut changed = false;
        if let Some(job) = &self.inspection {
            match job.try_recv() {
                Ok(Ok(vars)) => {
                    self.enabled = vec![false; vars.len()];
                    if !vars.is_empty() {
                        self.enabled[0] = true;
                    }
                    self.variables = vars;
                    self.inspection = None;
                    changed = true;
                }
                Ok(Err(e)) => {
                    self.status = Some(Err(e));
                    self.inspection = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = Some(Err("Product inspection stopped".into()));
                    self.inspection = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(job) = &self.coverage_job {
            match job.try_recv() {
                Ok(Ok(preview)) => {
                    self.coverage = Some(preview);
                    self.coverage_job = None;
                    changed = true;
                }
                Ok(Err(e)) => {
                    self.status = Some(Err(e));
                    self.coverage_job = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = Some(Err("Coverage preview stopped".into()));
                    self.coverage_job = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(job) = &self.export_job {
            match job.try_recv() {
                Ok(result) => {
                    self.status = Some(result);
                    self.export_job = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = Some(Err("Export worker stopped".into()));
                    self.export_job = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        changed
            || self.inspection.is_some()
            || self.coverage_job.is_some()
            || self.export_job.is_some()
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
    state.coverage_job = None;
    let paths: Arc<Vec<PathBuf>> = Arc::new(
        range
            .select(context.browser.products(state.source))
            .into_iter()
            .map(|product| source.root.join(&product.relative_path))
            .collect(),
    );
    if state.inspected.as_ref() != paths.first() {
        state.inspected = None;
        state.inspection = None;
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
fn editable_bounds(ui: &Ui, state: &mut State) {
    ui.text("Spatial bounds (degrees)");
    if let Some(_table) = ui.begin_table("subset_bounds", 4) {
        for label in ["West", "South", "East", "North"] {
            ui.table_setup_column_stretch_weight(label, TableColumnFlags::NONE, 1.0);
        }
        ui.table_headers_row();
        ui.table_next_row();
        for (label, field) in [
            ("west", &mut state.west),
            ("south", &mut state.south),
            ("east", &mut state.east),
            ("north", &mut state.north),
        ] {
            ui.table_next_column();
            ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
            ui.input_text(format!("##subset_{label}"), field).build();
        }
    }
}
fn draw_map(ui: &Ui, state: &mut State, settings: &Settings) {
    let Some(coverage) = &state.coverage else {
        return;
    };
    ui.text(format!(
        "Combined coverage: {} products, {} sampled coordinate points",
        coverage.products, coverage.sampled_points
    ));
    if ui.button(if state.preview_globe {
        "World map"
    } else {
        "Globe"
    }) {
        state.preview_globe = !state.preview_globe;
    }
    item_tip(
        ui,
        settings.tutorial_tips,
        "Switch between flat and globe coverage.",
        "Switch between a world map for drawing bounds and a globe for viewing coverage.",
    );
    let width = ui.content_region_avail()[0].min(680.0).max(220.0);
    let height = if state.preview_globe {
        width.min(400.0)
    } else {
        width * 0.5
    };
    let min = ui.cursor_screen_pos();
    let max = [min[0] + width, min[1] + height];
    ui.invisible_button("##subset_map", [width, height]);
    let draw = ui.get_window_draw_list();
    draw.add_rect(min, max, settings.palette.panel)
        .filled(true)
        .build();
    let bounds = state.bounds().ok();
    let max_count = coverage.counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    if state.preview_globe {
        let center = [min[0] + width * 0.5, min[1] + height * 0.5];
        let radius = width.min(height) * 0.43;
        if ui.is_item_active() && ui.is_mouse_dragging(dear_app::imgui::MouseButton::Left) {
            let delta = ui.io().mouse_delta();
            state.globe_yaw -= delta[0] as f64 / radius as f64;
            state.globe_pitch =
                (state.globe_pitch + delta[1] as f64 / radius as f64).clamp(-1.5, 1.5);
        }
        draw.add_circle(center, radius, settings.palette.border)
            .build();
        for y in 0..coverage.height {
            for x in 0..coverage.width {
                let count = coverage.counts[y * coverage.width + x];
                if count == 0 {
                    continue;
                }
                let lat = (90.0 - (y as f64 + 0.5)).to_radians();
                let lon = ((x as f64 + 0.5) - 180.0).to_radians() - state.globe_yaw;
                let depth = lat.sin() * state.globe_pitch.sin()
                    + lat.cos() * lon.cos() * state.globe_pitch.cos();
                if depth <= 0.0 {
                    continue;
                }
                let p = [
                    center[0] + radius * (lat.cos() * lon.sin()) as f32,
                    center[1]
                        - radius
                            * (lat.sin() * state.globe_pitch.cos()
                                - lat.cos() * lon.cos() * state.globe_pitch.sin())
                                as f32,
                ];
                let alpha = 0.3 + 0.7 * (count as f32 / max_count).sqrt();
                let c = settings.palette.accent;
                draw.add_circle(
                    p,
                    1.0 + (count as f32).sqrt().min(3.0),
                    [c[0], c[1], c[2], alpha],
                )
                .filled(true)
                .build();
            }
        }
    } else {
        let dx = width / coverage.width as f32;
        let dy = height / coverage.height as f32;
        for y in 0..coverage.height {
            for x in 0..coverage.width {
                let count = coverage.counts[y * coverage.width + x];
                if count == 0 {
                    continue;
                }
                let lat = 90.0 - (y as f64 + 0.5);
                let lon = (x as f64 + 0.5) - 180.0;
                let selected = bounds.is_some_and(|b| b.contains(lat, lon));
                let c = if selected {
                    settings.palette.accent
                } else {
                    settings.palette.heatmap_low
                };
                let alpha = 0.25 + 0.75 * (count as f32 / max_count).sqrt();
                draw.add_rect(
                    [min[0] + x as f32 * dx, min[1] + y as f32 * dy],
                    [
                        min[0] + (x + 1) as f32 * dx + 0.4,
                        min[1] + (y + 1) as f32 * dy + 0.4,
                    ],
                    [c[0], c[1], c[2], alpha],
                )
                .filled(true)
                .build();
            }
        }
        let mouse = ui.io().mouse_pos();
        if ui.is_item_hovered() && ui.is_mouse_clicked(dear_app::imgui::MouseButton::Left) {
            state.drag_start = Some(mouse);
        }
        if let Some(start) = state.drag_start {
            let clamped = [
                mouse[0].clamp(min[0], max[0]),
                mouse[1].clamp(min[1], max[1]),
            ];
            if ui.is_mouse_down(dear_app::imgui::MouseButton::Left) {
                draw.add_rect(
                    [start[0].min(clamped[0]), start[1].min(clamped[1])],
                    [start[0].max(clamped[0]), start[1].max(clamped[1])],
                    settings.palette.accent,
                )
                .thickness(2.0)
                .build();
            } else {
                if (start[0] - clamped[0]).abs() > 3.0 && (start[1] - clamped[1]).abs() > 3.0 {
                    let lon = |x: f32| -180.0 + 360.0 * ((x - min[0]) / width) as f64;
                    let lat = |y: f32| 90.0 - 180.0 * ((y - min[1]) / height) as f64;
                    state.west = format!("{:.3}", lon(start[0].min(clamped[0])));
                    state.east = format!("{:.3}", lon(start[0].max(clamped[0])));
                    state.north = format!("{:.3}", lat(start[1].min(clamped[1])));
                    state.south = format!("{:.3}", lat(start[1].max(clamped[1])));
                }
                state.drag_start = None;
            }
        }
    }
    item_tip(ui,settings.tutorial_tips,"Coverage count is the number of products touching a map cell.","Drag on the globe to rotate it, or drag a rectangle on the world map to set geographic bounds. Brighter cells are covered by more selected products.");
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
    let state = &mut lab.spatiotemporal_subset;
    state.is_busy();
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
    ui.text("Dataset Browser source");
    if ui.combo_simple_string("##subset_source", &mut state.source, &labels) {
        state.invalidate();
        state.status = None;
    }
    item_tip(ui,context.settings.tutorial_tips,"Use products already indexed by Dataset Browser.","Choose a registered Dataset Browser source. Product discovery and coordinates are shared with the Browser.");
    if let Some(_table) = ui.begin_table("subset_ranges", 2) {
        ui.table_setup_column_stretch_weight("Date range", TableColumnFlags::NONE, 1.0);
        ui.table_setup_column_stretch_weight("Orbit range", TableColumnFlags::NONE, 1.0);
        ui.table_next_row();
        ui.table_next_column();
        ui.text("Date range (YYYY-MM-DD)");
        ui.input_text("From##subset_date_from", &mut state.from_date)
            .build();
        ui.input_text("Through##subset_date_through", &mut state.through_date)
            .build();
        ui.table_next_column();
        ui.text("Orbit range");
        ui.input_text("From##subset_orbit_from", &mut state.from_orbit)
            .build();
        ui.input_text("Through##subset_orbit_through", &mut state.through_orbit)
            .build();
    }
    let products = selected_products(&context, state);
    ui.text(format!("{} indexed products selected", products.len()));
    if let Some(first) = products.first() {
        if state.inspected.as_ref() != Some(first) && state.inspection.is_none() {
            state.inspected = Some(first.clone());
            state.variables.clear();
            state.enabled.clear();
            state.coverage = None;
            let path = first.clone();
            let rules = context.settings.dataset_rules.clone();
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(inspect_subset_product_with_rules(&path, &rules));
            });
            state.inspection = Some(receiver);
        }
    }
    if state.inspection.is_some() {
        ui.text("Inspecting representative product...");
    } else if state.variables.is_empty() {
        ui.text_wrapped("No supported geographic numeric fields found. This first version accepts NetCDF regular grids and 2D swaths with identifiable latitude and longitude coordinates.");
    } else {
        ui.text("Variables");
        for (i, (variable, enabled)) in state.variables.iter().zip(&mut state.enabled).enumerate() {
            let label = format!(
                "{}  {:?}##subset_variable_{i}",
                variable.path, variable.shape
            );
            if ui.checkbox(label, enabled) {
                state.coverage = None;
                state.coverage_job = None;
            }
        }
        item_tip(ui,context.settings.tutorial_tips,"Select fields sharing the same spatial coordinates and shape.","Choose one or more variables from the representative product. The export checks each selected product for compatible fields.");
    }
    editable_bounds(ui, state);
    if state.coverage_job.is_some() {
        ui.text("Sampling combined coverage...");
    } else if ui.button("Preview selected coverage") {
        if let Some(choice) = selected_variables(state).first().cloned() {
            let paths = products.as_ref().clone();
            let max_points = context.settings.spatiotemporal_subset_preview_max_points;
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(preview_subset_coverage(&paths, &choice, max_points));
            });
            state.coverage_job = Some(receiver);
            state.coverage = None;
            state.status = None;
        } else {
            state.status = Some(Err("Select at least one variable".into()));
        }
    }
    draw_map(ui, state, context.settings);
    ui.separator();
    ui.text("Export view");
    if ui.radio_button_bool("Gridded mosaic", state.mode == SubsetMode::Mosaic) {
        state.mode = SubsetMode::Mosaic;
    }
    ui.same_line();
    if ui.radio_button_bool("Native products", state.mode == SubsetMode::Native) {
        state.mode = SubsetMode::Native;
    }
    item_tip(ui,context.settings.tutorial_tips,"Native keeps separate cropped products; mosaic averages overlap into one grid.","Choose Native products to retain each product's coordinates and time samples in one NetCDF file. Choose Gridded mosaic for one coverage-wide mean per variable and a sample-count field.");
    if state.mode == SubsetMode::Mosaic {
        ui.input_text("Grid resolution (degrees)", &mut state.resolution)
            .build();
    }
    ui.text("New NetCDF file");
    if file_path_field(
        ui,
        "subset_new_file",
        &mut state.output,
        ui.content_region_avail()[0],
    ) {
        *context.picker_request = Some((FilePickerTarget::SubsetOutput, state.output.clone()));
    }
    item_tip(ui,context.settings.tutorial_tips,"Existing output files are protected.","Enter an output .nc filename or absolute path. The exporter writes a new NetCDF-4 file and preserves existing files.");
    if state.export_job.is_some() {
        ui.text("Exporting in bounded chunks...");
    }
    if let Some(status) = &state.status {
        match status {
            Ok(s) => ui.text_wrapped(s),
            Err(e) => ui.text_wrapped(format!("Could not export: {e}")),
        }
    }
    let _disabled = ui.begin_disabled_with_cond(state.export_job.is_some());
    if ui.button("Export NetCDF-4") {
        let result = (|| {
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
            if products.is_empty() || variables.is_empty() {
                return Err("Select products and variables".into());
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
                resolution,
                output,
            })
        })();
        match result {
            Ok(request) => {
                let (sender, receiver) = mpsc::channel();
                thread::spawn(move || {
                    let _ = sender.send(export_subset(&request));
                });
                state.export_job = Some(receiver);
                state.status = None;
            }
            Err(e) => state.status = Some(Err(e)),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

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
                        picker_request: &mut picker_request
                    }
                )
                .is_none());
            });
        context.end_frame();
    }
}
