mod background_job;
mod data_toolkit;
mod dataset_browser;
mod dataset_picker;
mod details;
mod export;
mod file_drop;
mod file_path_field;
mod geo_texture;
mod jobs;
mod layout;
mod lifecycle;
mod model;
mod monitor;
#[path = "plugins/mod.rs"]
mod plugins;
mod settings;
mod state;
mod tutorial_tips;
mod ui;
mod value_visibility;
mod views;
pub(crate) use views::coastlines;
mod volume_raycast;

use crate::core::*;
use background_job::*;
use dataset_browser::*;
use dataset_picker::*;
use export::*;
use file_drop::*;
use file_path_field::*;
use geo_texture::*;
use model::*;
use monitor::*;
use settings::*;
use state::*;
use tutorial_tips::*;
use ui::*;
use views::*;
use volume_raycast::*;

use dear_app::{
    imgui::{
        ChildFlags, ColorEditFlags, ColorStackToken, Condition, DrawListMut, FontConfig,
        FontSource, ListClipper, MouseCursor, StyleColor, StyleVar, TabItemFlags, TableColumnFlags,
        TableColumnStateFlags, TableFlags, TableOptions, TableRowFlags, TableSizingPolicy, Ui,
        WindowFlags,
    },
    run as run_app, wgpu, AppConfig, Application, EventContext, FrameContext, InitContext,
    PrepareFrameContext, RunError, Theme,
};
use dear_implot::{
    AxisFlags, Colormap, HeatmapPlot, ImPlotExt, ImPlotPoint, ItemFlags, Plot, PlotCond,
    PlotContext, PlotUi, XAxis, YAxis,
};
use h5jive_data::{
    same_file, AttributeValue, CoordinateAxis, DatasetDistribution, DatasetStatistics, Entry, File,
    Geolocation, GeolocationRule, Kind, NumericEdit, OpenOptions, Region, ScanFilter, WritableFile,
};
use scinumtools3::Project;
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

const TABLE_CELL_PADDING: [f32; 2] = [8.0, 5.0];
const PANEL_PADDING: [f32; 2] = [8.0, 8.0];
const BROWSER_PANEL_PADDING: [f32; 2] = [16.0, 12.0];
const PANEL_GAP: f32 = 8.0;
const CONTROL_INLINE_GAP: f32 = 4.0;
const CONTROL_FRAME_PADDING: [f32; 2] = [6.0, 3.0];
const CONTROL_INNER_SPACING: [f32; 2] = [6.0, 4.0];
const VALUE_GRID_HEIGHT: f32 = 4.0 * 26.0 + 3.0 * PANEL_GAP;
const PLOT_NAV_HEIGHT: f32 = VALUE_GRID_HEIGHT + PANEL_GAP + 58.0;
const REFINEMENT_IDLE: Duration = Duration::from_millis(150);

fn volume_camera_distance(scale: [f32; 3]) -> f32 {
    (scale.iter().sum::<f32>() * 2.0).max(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlotNavAction {
    None,
    Reset,
    Center,
    ZoomOut,
    ZoomIn,
}

#[derive(Clone, Copy)]
struct VolumeRotation([f32; 4]);

enum MapJobUpdate {
    Preview(MapData),
    Complete(Result<(MapData, Option<Arc<Region>>), String>),
}

#[derive(Clone, Copy)]
struct GlobeRotation {
    start_yaw: f32,
    yaw_delta: f32,
    start_pitch: f32,
    target_pitch: f32,
    started_at: Instant,
}

enum ProfileJobUpdate {
    Overview(ProfileData),
    Complete(Result<ProfileData, String>),
}

#[derive(Clone, PartialEq, Eq)]
struct ProfileKey {
    selection: Selection,
    slice: ProfileSlice,
    visibility: ValueVisibility,
}

struct App {
    files: Vec<File>,
    file_paths: Vec<PathBuf>,
    file_path_inputs: [String; 2],
    file_error: Option<String>,
    file_open_jobs: [Option<mpsc::Receiver<Result<File, String>>>; 2],
    file_open_paths: [Option<PathBuf>; 2],
    pending_browser_b: Option<PathBuf>,
    file_picker: Option<FilePickerState>,
    file_drop: FileDropState,
    export_dialog: Option<ExportDialog>,
    pending_export: Option<PendingExport>,
    view_rect: [f32; 4],
    selection: Option<Selection>,
    sync_restore_selection: Option<Selection>,
    startup_tab: Option<usize>,
    details_key: Option<Selection>,
    details_default_pending: bool,
    statistics_cache: Option<Result<DatasetStatistics, String>>,
    statistics_job: BackgroundJob<Selection, DatasetStatistics>,
    distribution_cache: Option<Result<DatasetDistribution, String>>,
    distribution_job: BackgroundJob<Selection, DatasetDistribution>,
    attributes_cache: Option<Result<Vec<AttributeValue>, String>>,
    coordinates_cache: Option<Result<Vec<CoordinateAxis>, String>>,
    coordinate_value_cache: HashMap<(usize, usize), Option<String>>,
    tree_geolocation_cache: HashMap<(usize, String), Option<Geolocation>>,
    selection_key: Option<Selection>,
    axis_names: Vec<String>,
    region_cache: Vec<(RegionKey, Arc<Result<Region, String>>)>,
    pending_cell_edits: HashMap<CellKey, PendingCellEdit>,
    cell_editor: Option<CellEditor>,
    cell_edit_job: Option<CellEditJob>,
    cell_edit_message: Option<String>,
    queued_selection: Option<Selection>,
    heatmap_cache: Option<(HeatmapKey, Result<Arc<HeatmapImage>, String>)>,
    heatmap_tiles: Vec<(HeatmapKey, Arc<HeatmapImage>)>,
    heatmap_tile_job: Option<(HeatmapKey, mpsc::Receiver<Result<HeatmapImage, String>>)>,
    heatmap_refine_generation: Arc<AtomicU64>,
    heatmap_navigation_at: Option<Instant>,
    heatmap_overview_cache: Option<(HeatmapOverviewKey, Result<HeatmapImage, String>)>,
    heatmap_overview_job: Option<(
        HeatmapOverviewKey,
        mpsc::Receiver<Result<HeatmapImage, String>>,
    )>,
    heatmap_view_overview_cache: Option<(HeatmapOverviewKey, Result<HeatmapImage, String>)>,
    heatmap_view_overview_job: Option<(
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
    profile_cache: Option<(ProfileKey, Result<ProfileData, String>)>,
    profile_job: Option<(ProfileKey, mpsc::Receiver<ProfileJobUpdate>)>,
    profile_load_generation: Arc<AtomicU64>,
    volume_cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    volume_overview_cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    volume_refined_cache: Vec<(VolumeKey, Arc<ExactVolumeData>)>,
    volume_job: Option<(
        VolumeKey,
        mpsc::Receiver<Result<Arc<ExactVolumeData>, String>>,
    )>,
    volume_refine_generation: Arc<AtomicU64>,
    volume_raster_generation: Arc<AtomicU64>,
    volume_navigation_at: Option<Instant>,
    volume_raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    volume_overview_raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    volume_raster_job: Option<VolumeRasterJob>,
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
    map_job: Option<(MapKey, mpsc::Receiver<MapJobUpdate>)>,
    map_plane: Option<(MapPlaneKey, Arc<Region>)>,
    map_load_generation: Arc<AtomicU64>,
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
    globe_auto_center_source: Option<(usize, String)>,
    globe_auto_center_pending: bool,
    globe_rotation: Option<GlobeRotation>,
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
    plugins: plugins::PluginLab,
    dataset_browser: DatasetBrowser,
    browser_footer_height: f32,
    view: ViewMode,
    view_load_generation: Arc<AtomicU64>,
    value_visibility: ValueVisibility,
    volume_visibility: ValueVisibility,
    sidebar_width: f32,
    details_panel_fraction: f32,
    show_about: bool,
    show_section_help: bool,
    show_dataset_registration_help: bool,
    tutorial_tips_error: Option<String>,
    settings: Settings,
    resource_monitor: ResourceMonitor,
    gpu_initialized: Arc<AtomicBool>,
    wayland_session: bool,
    display_backend: Option<&'static str>,
}

impl Application for App {
    fn initialized(
        &mut self,
        _context: &mut dear_app::InitializedContext<'_>,
        _gpu: &mut dear_app::GpuContext<'_>,
    ) -> Result<(), RunError> {
        self.gpu_initialized.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        self.display_backend = display_backend_name(context.window(), self.wayland_session);
        context
            .imgui()
            .font_atlas()
            .add_font(&[FontSource::default_vector_with_size(13.0)
                .with_config(FontConfig::new().rasterizer_density(2.0))]);
        Ok(())
    }

    fn event(&mut self, context: &mut EventContext<'_>) -> Result<(), RunError> {
        if context.window_id() != context.main_window().id() {
            return Ok(());
        }
        match self
            .file_drop
            .handle_event(context.event(), context.window())
        {
            DropAction::Open(slot, path) => self.open_dropped_file(slot, path),
            DropAction::UnknownPosition => {
                self.file_error = Some("Could not determine the drop target. Use Browse A or Browse B to open the file.".into());
            }
            DropAction::None => {}
        }
        Ok(())
    }

    fn prepare_frame(&mut self, context: &mut PrepareFrameContext<'_>) -> Result<(), RunError> {
        if self.file_drop.hovering {
            self.file_drop.refresh_pointer(context.window());
        }
        Ok(())
    }

    fn frame(&mut self, frame: &mut FrameContext<'_>) -> Result<(), RunError> {
        if self.file_drop.hovering {
            let _ = frame
                .wake_handle()
                .request_redraw_at(Instant::now() + Duration::from_millis(33));
        }
        self.advance_export();
        self.ensure_earth_texture(frame.gpu());
        if self.view == ViewMode::Spatial3d {
            self.poll_volume_raster(frame.gpu());
        }
        let ui = frame.ui();
        let plot_ctx = frame.addons().implot().expect("ImPlot add-on enabled");
        let plot_ui = ui.implot(plot_ctx);
        let frame_start = Instant::now();
        self.draw(ui, &plot_ui, plot_ctx);
        self.resource_monitor.record_ui_time(frame_start.elapsed());
        if self.view == ViewMode::Globe && self.globe_rotation.is_some() {
            let _ = frame
                .wake_handle()
                .request_redraw_at(Instant::now() + Duration::from_millis(16));
        }
        let _ = frame
            .wake_handle()
            .request_redraw_at(self.resource_monitor.next_sample_at());
        if self.pending_export.is_some() {
            let _ = frame.wake_handle().request_redraw_at(
                std::time::Instant::now() + std::time::Duration::from_millis(80),
            );
        }
        if self.plugins.is_busy()
            || self.cell_edit_job.is_some()
            || self.heatmap_overview_job.is_some()
            || self.heatmap_view_overview_job.is_some()
            || self.profile_job.is_some()
            || self.volume_raster_job.is_some()
            || self.map_job.is_some()
        {
            let _ = frame.wake_handle().request_redraw_at(
                std::time::Instant::now() + std::time::Duration::from_millis(80),
            );
        }
        for activity in [self.heatmap_navigation_at, self.volume_navigation_at]
            .into_iter()
            .flatten()
        {
            let deadline = activity + REFINEMENT_IDLE;
            if deadline > Instant::now() {
                let _ = frame.wake_handle().request_redraw_at(deadline);
            }
        }
        Ok(())
    }
}

struct FilePickerEntry {
    path: PathBuf,
    label: String,
    is_directory: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilePickerTarget {
    Input(usize),
    ViewExport,
    ManipulationOutput,
    SubsetOutput,
}

impl FilePickerTarget {
    fn is_save(self) -> bool {
        !matches!(self, Self::Input(_))
    }
    fn title(self) -> &'static str {
        match self {
            Self::Input(0) => "Open File A",
            Self::Input(_) => "Open File B",
            Self::ViewExport => "Choose image path",
            Self::ManipulationOutput => "Choose HDF5 path",
            Self::SubsetOutput => "Choose NetCDF path",
        }
    }
    fn accepts(self, path: &std::path::Path) -> bool {
        if let Self::Input(_) = self {
            return layout::is_supported_input_file(path);
        }
        match path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png" | "jpg" | "jpeg") if self == Self::ViewExport => true,
            Some("h5" | "hdf5") if self == Self::ManipulationOutput => true,
            Some("nc") if self == Self::SubsetOutput => true,
            _ => false,
        }
    }
}

struct FilePickerState {
    target: FilePickerTarget,
    needs_open: bool,
    directory: PathBuf,
    filename: String,
    entries: Vec<FilePickerEntry>,
    selected: Option<PathBuf>,
    pending_delete: Option<PathBuf>,
    error: Option<String>,
    creating_folder: bool,
    new_folder_name: String,
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-v") {
        println!("H5Jive v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("H5Jive [--inspect] [--view-VIEW] [FILE_A[:/DATASET]] [FILE_B[:/DATASET]]\nH5Jive --settings-help DIPL_PATH\nH5Jive -v|--version\nVIEW: data, 1d-profiles, 2d-slice, 3d, world-map, globe (requires :/DATASET)\nReads HDF5 and NetCDF files. DIPL: settings/runtime/DIPfile in source or settings/DIPfile in installs; overrides.dip beside it");
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--settings-help") {
        if args.len() != 2 {
            return Err("--settings-help requires one DIPL path".into());
        }
        println!("{}", settings_help(&args[1])?);
        return Ok(());
    }
    let (inspect, view_override, inputs) = parse_startup_args(&args)?;
    let wayland_session = was_wayland_session();
    if !inspect {
        prefer_xwayland_for_file_drops();
    }
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
    let retry_with_gl = cfg!(target_os = "linux")
        && settings.graphics_backend.is_none()
        && env::var_os("WGPU_BACKEND").is_none()
        && env::var_os("H5JIVE_GL_RETRY").is_none()
        && config
            .wgpu
            .backends
            .contains(wgpu::Backends::VULKAN | wgpu::Backends::GL);
    let gpu_initialized = Arc::new(AtomicBool::new(false));
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
        file_drop: FileDropState::default(),
        export_dialog: None,
        pending_export: None,
        view_rect: [0.0; 4],
        startup_tab: selection.as_ref().map(|selected| selected.file),
        selection,
        sync_restore_selection: None,
        details_key: None,
        details_default_pending: true,
        statistics_cache: None,
        statistics_job: BackgroundJob::default(),
        distribution_cache: None,
        distribution_job: BackgroundJob::default(),
        attributes_cache: None,
        coordinates_cache: None,
        coordinate_value_cache: HashMap::new(),
        tree_geolocation_cache: HashMap::new(),
        selection_key: None,
        axis_names: Vec::new(),
        region_cache: Vec::new(),
        pending_cell_edits: HashMap::new(),
        cell_editor: None,
        cell_edit_job: None,
        cell_edit_message: None,
        queued_selection: None,
        heatmap_cache: None,
        heatmap_tiles: Vec::new(),
        heatmap_tile_job: None,
        heatmap_refine_generation: Arc::new(AtomicU64::new(0)),
        heatmap_navigation_at: None,
        heatmap_overview_cache: None,
        heatmap_overview_job: None,
        heatmap_view_overview_cache: None,
        heatmap_view_overview_job: None,
        heatmap_range_cache: None,
        heatmap_tick_view: None,
        heatmap_range_job: None,
        heatmap_colormap: None,
        profile_cache: None,
        profile_job: None,
        profile_load_generation: Arc::new(AtomicU64::new(0)),
        volume_cache: None,
        volume_overview_cache: None,
        volume_refined_cache: Vec::new(),
        volume_job: None,
        volume_refine_generation: Arc::new(AtomicU64::new(0)),
        volume_raster_generation: Arc::new(AtomicU64::new(0)),
        volume_navigation_at: None,
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
        map_plane: None,
        map_load_generation: Arc::new(AtomicU64::new(0)),
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
        globe_auto_center_source: None,
        globe_auto_center_pending: false,
        globe_rotation: None,
        draft_slice: None,
        applied_slice: None,
        draft_table: None,
        applied_table: None,
        draft_profile: None,
        applied_profile: None,
        table_column_page: 0,
        table_footer_height: 150.0,
        slice_footer_height: 150.0,
        profile_footer_height: 170.0,
        profile_zoom: 1.0,
        heatmap_nav_override: None,
        view_selector_height: settings.view_button_height + 4.0,
        workspace_mode: WorkspaceMode::DataInspector,
        plugins: plugins::PluginLab::default(),
        dataset_browser: DatasetBrowser::new(&settings.dataset_sources),
        browser_footer_height: 105.0,
        view: view_override.unwrap_or(settings.default_view),
        view_load_generation: Arc::new(AtomicU64::new(0)),
        value_visibility: ValueVisibility::default(),
        volume_visibility: ValueVisibility {
            fill: false,
            ..ValueVisibility::default()
        },
        sidebar_width: settings.sidebar_width,
        details_panel_fraction: settings.details_panel_fraction,
        resource_monitor: ResourceMonitor::new(),
        gpu_initialized: Arc::clone(&gpu_initialized),
        wayland_session,
        display_backend: None,
        show_about: false,
        show_section_help: false,
        show_dataset_registration_help: false,
        tutorial_tips_error: None,
        settings,
    };
    match run_app(config, app) {
        Ok(()) => Ok(()),
        Err(error) => {
            if retry_with_gl
                && !gpu_initialized.load(Ordering::Relaxed)
                && matches!(
                    error,
                    RunError::SurfaceCreation(_)
                        | RunError::AdapterUnavailable(_)
                        | RunError::SurfaceFormatUnavailable
                        | RunError::DeviceRequest(_)
                        | RunError::RendererInit(_)
                )
            {
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;

                    eprintln!("Graphics startup failed ({error}); retrying with OpenGL.");
                    let exec_error = std::process::Command::new(env::current_exe()?)
                        .args(env::args_os().skip(1))
                        .env("WGPU_BACKEND", "gl")
                        .env("H5JIVE_GL_RETRY", "1")
                        .exec();
                    return Err(format!("OpenGL retry could not start: {exec_error}").into());
                }
            }
            Err(error.into())
        }
    }
}

#[cfg(target_os = "linux")]
fn prefer_xwayland_for_file_drops() {
    // dear-app constructs winit's event loop internally. Winit 0.30 chooses
    // Wayland before X11 when both displays are advertised, but its Wayland
    // backend does not deliver file-drop events.
    if env::var("DISPLAY").is_ok_and(|display| !display.is_empty())
        && env::var("H5JIVE_WINDOW_BACKEND").as_deref() != Ok("wayland")
    {
        env::remove_var("WAYLAND_DISPLAY");
        env::remove_var("WAYLAND_SOCKET");
    }
}

#[cfg(not(target_os = "linux"))]
fn prefer_xwayland_for_file_drops() {}

#[cfg(target_os = "linux")]
fn was_wayland_session() -> bool {
    ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        .iter()
        .any(|name| env::var(name).is_ok_and(|value| !value.is_empty()))
        || env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
}

#[cfg(not(target_os = "linux"))]
fn was_wayland_session() -> bool {
    false
}

fn display_backend_name(
    window: &winit::window::Window,
    wayland_session: bool,
) -> Option<&'static str> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::AppKit(_) => Some("WindowServer"),
        RawWindowHandle::Win32(_) => Some("Windows"),
        RawWindowHandle::Wayland(_) => Some("Wayland"),
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) if wayland_session => Some("XWayland"),
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => Some("X11"),
        _ => None,
    }
}

fn parse_startup_args(
    args: &[String],
) -> Result<(bool, Option<ViewMode>, Vec<(PathBuf, Option<String>)>), String> {
    let mut inspect = false;
    let mut view = None;
    let mut inputs = Vec::new();
    let mut literal_paths = false;
    for arg in args {
        if !literal_paths && arg == "--" {
            literal_paths = true;
            continue;
        }
        if !literal_paths && arg == "--inspect" {
            inspect = true;
            continue;
        }
        if !literal_paths && arg.starts_with("--view-") {
            let requested = match arg.as_str() {
                "--view-data" => ViewMode::Data,
                "--view-1d-profiles" => ViewMode::Profiles1d,
                "--view-2d-slice" => ViewMode::Slice2d,
                "--view-3d" => ViewMode::Spatial3d,
                "--view-world-map" => ViewMode::WorldMap,
                "--view-globe" => ViewMode::Globe,
                _ => return Err(format!("unknown view option: {arg}")),
            };
            if view.replace(requested).is_some() {
                return Err("specify only one --view-* option".into());
            }
            continue;
        }
        if !literal_paths && arg.starts_with('-') {
            return Err(format!("unknown option: {arg}"));
        }
        inputs.push(parse_input_spec(arg)?);
    }
    if inputs.len() > 2 {
        return Err("expected at most two file paths".into());
    }
    if view.is_some() && !inputs.iter().any(|(_, dataset)| dataset.is_some()) {
        return Err("a --view-* option requires FILE:/DATASET".into());
    }
    if inspect && view.is_some() {
        return Err("--inspect cannot be combined with --view-*".into());
    }
    Ok((inspect, view, inputs))
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
