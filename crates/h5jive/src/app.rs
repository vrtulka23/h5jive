mod data_toolkit;
mod dataset_browser;
mod details;
mod export;
mod geo_texture;
mod jobs;
mod layout;
mod model;
mod settings;
mod ui;
mod value_visibility;
mod views;
mod volume_raycast;

use crate::core::*;
use dataset_browser::*;
use export::*;
use geo_texture::*;
use model::*;
use settings::*;
use ui::*;
use views::*;
use volume_raycast::*;

use dear_app::{
    imgui::{
        ColorEditFlags, ColorStackToken, Condition, DrawListMut, FontConfig, FontSource,
        ListClipper, MouseCursor, StyleColor, StyleVar, TabItemFlags, TableColumnFlags,
        TableColumnStateFlags, TableFlags, TableOptions, TableSizingPolicy, Ui, WindowFlags,
    },
    run as run_app, wgpu, AppConfig, Application, FrameContext, InitContext, RunError, Theme,
};
use dear_implot::{
    AxisFlags, Colormap, HeatmapPlot, ImPlotExt, ImPlotPoint, ItemFlags, Plot, PlotCond,
    PlotContext, PlotUi, XAxis, YAxis,
};
use h5jive_data::{
    AttributeValue, CoordinateAxis, DatasetDistribution, DatasetStatistics, Entry, File,
    Geolocation, GeolocationRule, Kind, OpenOptions, Region, ScanFilter,
};
use scinumtools3::Project;
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
};

const TABLE_CELL_PADDING: [f32; 2] = [8.0, 5.0];
const PANEL_PADDING: [f32; 2] = [8.0, 8.0];
const PANEL_GAP: f32 = 8.0;
const CONTROL_INLINE_GAP: f32 = 4.0;
const CONTROL_FRAME_PADDING: [f32; 2] = [6.0, 3.0];
const CONTROL_INNER_SPACING: [f32; 2] = [6.0, 4.0];
const VALUE_GRID_HEIGHT: f32 = 4.0 * 26.0 + 3.0 * PANEL_GAP;
const PLOT_NAV_HEIGHT: f32 = VALUE_GRID_HEIGHT + PANEL_GAP + 58.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlotNavAction {
    None,
    Reset,
    ZoomOut,
    ZoomIn,
}

#[derive(Clone, Copy)]
struct VolumeRotation([f32; 4]);

struct App {
    files: Vec<File>,
    file_paths: Vec<PathBuf>,
    file_path_inputs: [String; 2],
    file_error: Option<String>,
    file_open_jobs: [Option<mpsc::Receiver<Result<File, String>>>; 2],
    file_open_paths: [Option<PathBuf>; 2],
    pending_browser_b: Option<PathBuf>,
    file_picker: Option<FilePickerState>,
    export_dialog: Option<ExportDialog>,
    pending_export: Option<PendingExport>,
    view_rect: [f32; 4],
    selection: Option<Selection>,
    startup_tab: Option<usize>,
    details_key: Option<(usize, String)>,
    details_default_pending: bool,
    statistics_cache: Option<Result<DatasetStatistics, String>>,
    statistics_job: Option<mpsc::Receiver<Result<DatasetStatistics, String>>>,
    distribution_cache: Option<Result<DatasetDistribution, String>>,
    distribution_job: Option<mpsc::Receiver<Result<DatasetDistribution, String>>>,
    attributes_cache: Option<Result<Vec<AttributeValue>, String>>,
    coordinates_cache: Option<Result<Vec<CoordinateAxis>, String>>,
    coordinate_value_cache: HashMap<(usize, usize), Option<String>>,
    tree_geolocation_cache: HashMap<(usize, String), Option<Geolocation>>,
    selection_key: Option<(usize, String)>,
    axis_names: Vec<String>,
    region_cache: Vec<(RegionKey, Result<Region, String>)>,
    heatmap_cache: Option<(HeatmapKey, Result<Arc<HeatmapImage>, String>)>,
    heatmap_tiles: Vec<(HeatmapKey, Arc<HeatmapImage>)>,
    heatmap_tile_job: Option<(HeatmapKey, mpsc::Receiver<Result<HeatmapImage, String>>)>,
    heatmap_overview_cache: Option<(HeatmapOverviewKey, Result<HeatmapImage, String>)>,
    heatmap_overview_job: Option<(
        HeatmapOverviewKey,
        mpsc::Receiver<Result<HeatmapImage, String>>,
    )>,
    heatmap_range_cache: Option<(HeatmapRangeKey, Result<Option<(f64, f64)>, String>)>,
    heatmap_tick_view: Option<(HeatmapRangeKey, [f64; 4])>,
    heatmap_range_job: Option<(
        HeatmapRangeKey,
        mpsc::Receiver<Result<Option<(f64, f64)>, String>>,
    )>,
    heatmap_colormap: Option<([[f32; 4]; 3], String)>,
    profile_cache: Option<Result<ProfileData, String>>,
    volume_cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    volume_overview_cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    volume_refined_cache: Vec<(VolumeKey, Arc<ExactVolumeData>)>,
    volume_job: Option<(
        VolumeKey,
        mpsc::Receiver<Result<Arc<ExactVolumeData>, String>>,
    )>,
    volume_raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    volume_overview_raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    volume_raster_job: Option<(VolumeRasterKey, mpsc::Receiver<Vec<u8>>)>,
    draft_volume: Option<VolumeSlice>,
    applied_volume: Option<VolumeSlice>,
    draft_volume_opacity: f32,
    volume_opacity: f32,
    draft_volume_log_scale: bool,
    volume_log_scale: bool,
    volume_projection: Option<VolumeProjection>,
    volume_axis_corner: [f32; 3],
    volume_rotation: VolumeRotation,
    volume_zoom: f32,
    volume_pan: [f32; 2],
    volume_range_drag: Option<(usize, bool)>,
    volume_footer_height: f32,
    map_cache: Option<(MapKey, Result<MapData, String>)>,
    map_job: Option<(MapKey, mpsc::Receiver<Result<MapData, String>>)>,
    map_controls: GeoViewControls,
    globe_controls: GeoViewControls,
    selected_pics: usize,
    selected_site_group: Option<String>,
    map_viewport: MapViewport,
    map_footer_height: f32,
    globe_footer_height: f32,
    earth_texture: Option<EarthTexture>,
    globe_yaw: f32,
    globe_pitch: f32,
    globe_zoom: f32,
    draft_slice: Option<TableSlice>,
    applied_slice: Option<TableSlice>,
    draft_table: Option<TableSlice>,
    applied_table: Option<TableSlice>,
    draft_profile: Option<ProfileSlice>,
    applied_profile: Option<ProfileSlice>,
    table_column_page: usize,
    table_footer_height: f32,
    slice_footer_height: f32,
    profile_footer_height: f32,
    profile_zoom: f32,
    heatmap_nav_override: Option<(HeatmapRangeKey, [f64; 4])>,
    view_selector_height: f32,
    workspace_mode: WorkspaceMode,
    dataset_browser: DatasetBrowser,
    browser_footer_height: f32,
    view: ViewMode,
    view_load_generation: Arc<AtomicU64>,
    value_visibility: ValueVisibility,
    volume_visibility: ValueVisibility,
    sidebar_width: f32,
    details_panel_fraction: f32,
    show_about: bool,
    settings: Settings,
}

impl Application for App {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        context
            .imgui()
            .font_atlas()
            .add_font(&[FontSource::default_vector_with_size(13.0)
                .with_config(FontConfig::new().rasterizer_density(2.0))]);
        Ok(())
    }

    fn frame(&mut self, frame: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.advance_export();
        self.ensure_earth_texture(frame.gpu());
        if self.view == ViewMode::Spatial3d {
            self.poll_volume_raster(frame.gpu());
        }
        let ui = frame.ui();
        let plot_ctx = frame.addons().implot().expect("ImPlot add-on enabled");
        let plot_ui = ui.implot(plot_ctx);
        self.draw(ui, &plot_ui, plot_ctx);
        if self.pending_export.is_some() {
            let _ = frame.wake_handle().request_redraw_at(
                std::time::Instant::now() + std::time::Duration::from_millis(80),
            );
        }
        Ok(())
    }
}

struct FilePickerEntry {
    path: PathBuf,
    label: String,
    is_directory: bool,
}

struct FilePickerState {
    slot: usize,
    directory: PathBuf,
    entries: Vec<FilePickerEntry>,
    selected: Option<PathBuf>,
    error: Option<String>,
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--help" || s == "-h") {
        println!("H5Jive [--inspect] [FILE_A[:/DATASET]] [FILE_B[:/DATASET]]\nReads HDF5 and NetCDF files. DIPL project: settings/DIPfile; local overrides: settings/overrides.dip");
        return Ok(());
    }
    let inspect = args.first().is_some_and(|s| s == "--inspect");
    if inspect {
        args.remove(0);
    }
    if args.len() > 2 {
        return Err("expected at most two file paths".into());
    }
    let inputs = args
        .iter()
        .map(|arg| parse_input_spec(arg))
        .collect::<Result<Vec<_>, _>>()?;
    let settings = Settings::load()?;
    let files: Vec<File> = inputs
        .iter()
        .map(|(path, _)| {
            File::open_with_options(
                path,
                OpenOptions {
                    max_depth: settings.tree_max_depth,
                    max_objects: settings.tree_max_objects,
                },
            )
        })
        .collect::<Result<_, _>>()?;
    let selection = resolve_startup_selection(&files, &inputs)?;
    if inspect {
        for (i, file) in files.iter().enumerate() {
            println!("File {}: {}", i + 1, file.name);
            print_tree(&file.tree, 0);
            if let Some(path) = &inputs[i].1 {
                println!("Selected dataset: {path}");
            }
        }
        return Ok(());
    }
    let mut config = AppConfig {
        window_title: format!("{} v{}", settings.title, env!("CARGO_PKG_VERSION")),
        theme: Some(settings.profiles[settings.active_profile].base),
        clear_color: settings.palette.background,
        ..Default::default()
    };
    config.addons.with_implot = true;
    config.wgpu.backends = settings
        .graphics_backend
        .unwrap_or_else(|| default_wgpu_backends(settings.forwarded_x11_backend))
        .with_env();
    let app = App {
        files,
        file_paths: inputs.iter().map(|(path, _)| path.clone()).collect(),
        file_path_inputs: [
            inputs
                .first()
                .map_or_else(String::new, |(path, _)| path.display().to_string()),
            inputs
                .get(1)
                .map_or_else(String::new, |(path, _)| path.display().to_string()),
        ],
        file_error: None,
        file_open_jobs: [None, None],
        file_open_paths: [None, None],
        pending_browser_b: None,
        file_picker: None,
        export_dialog: None,
        pending_export: None,
        view_rect: [0.0; 4],
        startup_tab: selection.as_ref().map(|selected| selected.file),
        selection,
        details_key: None,
        details_default_pending: true,
        statistics_cache: None,
        statistics_job: None,
        distribution_cache: None,
        distribution_job: None,
        attributes_cache: None,
        coordinates_cache: None,
        coordinate_value_cache: HashMap::new(),
        tree_geolocation_cache: HashMap::new(),
        selection_key: None,
        axis_names: Vec::new(),
        region_cache: Vec::new(),
        heatmap_cache: None,
        heatmap_tiles: Vec::new(),
        heatmap_tile_job: None,
        heatmap_overview_cache: None,
        heatmap_overview_job: None,
        heatmap_range_cache: None,
        heatmap_tick_view: None,
        heatmap_range_job: None,
        heatmap_colormap: None,
        profile_cache: None,
        volume_cache: None,
        volume_overview_cache: None,
        volume_refined_cache: Vec::new(),
        volume_job: None,
        volume_raster: None,
        volume_overview_raster: None,
        volume_raster_job: None,
        draft_volume: None,
        applied_volume: None,
        draft_volume_opacity: settings.volume_default_opacity,
        volume_opacity: settings.volume_default_opacity,
        draft_volume_log_scale: settings.volume_default_log_scale,
        volume_log_scale: settings.volume_default_log_scale,
        volume_projection: Some(VolumeProjection::Isometric),
        volume_axis_corner: [0.5; 3],
        volume_rotation: VolumeRotation::from_angles(0.65, 0.45),
        volume_zoom: 1.0,
        volume_pan: [0.0, 0.0],
        volume_range_drag: None,
        volume_footer_height: 260.0,
        map_cache: None,
        map_job: None,
        map_controls: GeoViewControls::new(settings.map_show_pics)
            .with_dummy_rgb(settings.palette.accent),
        globe_controls: GeoViewControls::new(settings.map_show_pics)
            .with_dummy_rgb(settings.palette.accent),
        selected_pics: 0,
        selected_site_group: None,
        map_viewport: MapViewport::default(),
        map_footer_height: 0.0,
        globe_footer_height: 0.0,
        earth_texture: None,
        globe_yaw: 0.0,
        globe_pitch: 0.0,
        globe_zoom: 1.0,
        draft_slice: None,
        applied_slice: None,
        draft_table: None,
        applied_table: None,
        draft_profile: None,
        applied_profile: None,
        table_column_page: 0,
        table_footer_height: 110.0,
        slice_footer_height: 150.0,
        profile_footer_height: 170.0,
        profile_zoom: 1.0,
        heatmap_nav_override: None,
        view_selector_height: settings.view_button_height + 4.0,
        workspace_mode: WorkspaceMode::DataInspector,
        dataset_browser: DatasetBrowser::new(&settings.dataset_sources),
        browser_footer_height: 105.0,
        view: settings.default_view,
        view_load_generation: Arc::new(AtomicU64::new(0)),
        value_visibility: ValueVisibility::default(),
        volume_visibility: ValueVisibility {
            fill: false,
            ..ValueVisibility::default()
        },
        sidebar_width: settings.sidebar_width,
        details_panel_fraction: settings.details_panel_fraction,
        show_about: false,
        settings,
    };
    run_app(config, app)?;
    Ok(())
}
fn parse_input_spec(arg: &str) -> Result<(PathBuf, Option<String>), String> {
    if let Some((file, dataset)) = arg.rsplit_once(":/") {
        // A bare C:/path is a Windows file path, not a dataset suffix.
        if file.len() == 1 && file.as_bytes()[0].is_ascii_alphabetic() {
            return Ok((PathBuf::from(arg), None));
        }
        if file.is_empty() || dataset.is_empty() {
            return Err(format!("expected FILE:/DATASET, got {arg:?}"));
        }
        Ok((PathBuf::from(file), Some(format!("/{dataset}"))))
    } else {
        Ok((PathBuf::from(arg), None))
    }
}

fn resolve_startup_selection(
    files: &[File],
    inputs: &[(PathBuf, Option<String>)],
) -> Result<Option<Selection>, String> {
    let mut selected = None;
    for (index, (_, dataset)) in inputs.iter().enumerate() {
        let Some(path) = dataset else { continue };
        let entry = find_entry(&files[index].tree, path).ok_or_else(|| {
            format!(
                "dataset {path:?} was not found in {} (check the path and tree limits)",
                files[index].name
            )
        })?;
        if !matches!(entry.kind, Kind::Dataset { .. }) {
            return Err(format!(
                "{path:?} in {} is not a dataset",
                files[index].name
            ));
        }
        if selected.is_none() {
            selected = Some(Selection {
                file: index,
                path: path.clone(),
            });
        }
    }
    Ok(selected)
}
fn print_tree(entry: &Entry, depth: usize) {
    if depth > 0 {
        println!("{}{}", "  ".repeat(depth), entry.name);
    }
    for child in &entry.children {
        print_tree(child, depth + 1);
    }
}

#[cfg(test)]
mod tests;
