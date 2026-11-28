//! State groups owned by the application shell.
use super::*;

/// Loaded files, selection, file dialogs, and session-scoped notices.
pub(super) struct SessionState {
    pub(super) files: Vec<File>,
    pub(super) file_paths: Vec<PathBuf>,
    pub(super) file_path_inputs: [String; 2],
    pub(super) notices: NoticeCenter,
    pub(super) file_open_jobs: [BackgroundJob<PathBuf, File>; 2],
    pub(super) pending_browser_b: Option<PathBuf>,
    pub(super) file_picker: Option<FilePickerState>,
    pub(super) file_drop: FileDropState,
    pub(super) export_dialog: Option<ExportDialog>,
    pub(super) pending_export: Option<PendingExport>,
    pub(super) view_rect: [f32; 4],
    pub(super) selection: Option<Selection>,
    pub(super) sync_restore_selection: Option<Selection>,
    pub(super) startup_tab: Option<usize>,
    pub(super) queued_selection: Option<Selection>,
}

impl SessionState {
    pub(super) fn new(
        files: Vec<File>,
        inputs: &[(PathBuf, Option<String>)],
        selection: Option<Selection>,
    ) -> Self {
        let startup_tab = selection.as_ref().map(|selected| selected.file);
        Self {
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
            notices: NoticeCenter::default(),
            file_open_jobs: std::array::from_fn(|_| BackgroundJob::default()),
            pending_browser_b: None,
            file_picker: None,
            file_drop: FileDropState::default(),
            export_dialog: None,
            pending_export: None,
            view_rect: [0.0; 4],
            selection,
            sync_restore_selection: None,
            startup_tab,
            queued_selection: None,
        }
    }
}

/// Active workspace, view selection, navigation, and help flags.
pub(super) struct WorkspaceState {
    pub(super) view_selector_height: f32,
    pub(super) workspace_mode: WorkspaceMode,
    pub(super) plugins: plugins::PluginLab,
    pub(super) dataset_browser: DatasetBrowser,
    pub(super) browser_footer_height: f32,
    pub(super) view: ViewMode,
    pub(super) view_load_generation: Arc<AtomicU64>,
    pub(super) value_visibility: ValueVisibility,
    pub(super) sidebar_width: f32,
    pub(super) details_panel_fraction: f32,
    pub(super) show_about: bool,
    pub(super) show_section_help: bool,
    pub(super) show_dataset_registration_help: bool,
}

impl WorkspaceState {
    pub(super) fn new(settings: &Settings, view_override: Option<ViewMode>) -> Self {
        Self {
            view_selector_height: settings.view_button_height + 4.0,
            workspace_mode: WorkspaceMode::DataInspector,
            plugins: plugins::PluginLab::default(),
            dataset_browser: DatasetBrowser::new(&settings.dataset_sources),
            browser_footer_height: 105.0,
            view: view_override.unwrap_or(settings.default_view),
            view_load_generation: Arc::new(AtomicU64::new(0)),
            value_visibility: ValueVisibility::default(),
            sidebar_width: settings.sidebar_width,
            details_panel_fraction: settings.details_panel_fraction,
            show_about: false,
            show_section_help: false,
            show_dataset_registration_help: false,
        }
    }
}

/// Selected dataset metadata, inspector caches, edits, and search state.
#[derive(Default)]
pub(super) struct DatasetState {
    pub(super) details_key: Option<Selection>,
    pub(super) details_default_pending: bool,
    pub(super) statistics_cache: Option<Result<DatasetStatistics, String>>,
    pub(super) statistics_job: BackgroundJob<Selection, DatasetStatistics>,
    pub(super) distribution_cache: Option<Result<DatasetDistribution, String>>,
    pub(super) distribution_job: BackgroundJob<Selection, DatasetDistribution>,
    pub(super) attributes_cache: Option<Result<Vec<AttributeValue>, String>>,
    pub(super) coordinates_cache: Option<Result<Vec<CoordinateAxis>, String>>,
    pub(super) coordinate_value_cache: HashMap<(usize, usize), Option<String>>,
    pub(super) tree_geolocation_cache: HashMap<(usize, String), Option<Geolocation>>,
    pub(super) selection_key: Option<Selection>,
    pub(super) axis_names: Vec<String>,
    pub(super) region_cache: Vec<(RegionKey, Arc<Result<Region, String>>)>,
    pub(super) pending_cell_edits: HashMap<CellKey, PendingCellEdit>,
    pub(super) cell_editor: Option<CellEditor>,
    pub(super) cell_edit_job: Option<CellEditJob>,
    pub(super) find_replace_dialog: Option<FindReplaceDialog>,
    pub(super) bulk_replace_job: Option<BulkReplaceJob>,
    pub(super) table_scroll_to_row: Option<usize>,
    pub(super) draft_table: Option<TableSlice>,
    pub(super) applied_table: Option<TableSlice>,
    pub(super) table_column_page: usize,
    pub(super) table_footer_height: f32,
}

impl DatasetState {
    pub(super) fn new() -> Self {
        Self {
            details_default_pending: true,
            table_footer_height: 150.0,
            ..Self::default()
        }
    }
}

/// 2D heatmap and slice state.
#[derive(Default)]
pub(super) struct HeatmapState {
    pub(super) cache: Option<(HeatmapKey, Result<Arc<HeatmapImage>, String>)>,
    pub(super) tiles: Vec<(HeatmapKey, Arc<HeatmapImage>)>,
    pub(super) tile_job: BackgroundJob<HeatmapKey, HeatmapImage>,
    pub(super) refine_generation: Arc<AtomicU64>,
    pub(super) navigation_at: Option<Instant>,
    pub(super) overview_cache: Option<(HeatmapOverviewKey, Result<HeatmapImage, String>)>,
    pub(super) overview_job: BackgroundJob<HeatmapOverviewKey, HeatmapImage>,
    pub(super) view_overview_cache: Option<(HeatmapOverviewKey, Result<HeatmapImage, String>)>,
    pub(super) view_overview_job: BackgroundJob<HeatmapOverviewKey, HeatmapImage>,
    pub(super) range_cache: Option<(HeatmapRangeKey, Result<Option<(f64, f64)>, String>)>,
    pub(super) tick_view: Option<(HeatmapRangeKey, [f64; 4])>,
    pub(super) range_job: BackgroundJob<HeatmapRangeKey, Option<(f64, f64)>>,
    pub(super) colormap: Option<([[f32; 4]; 3], String)>,
    pub(super) nav_override: Option<(HeatmapRangeKey, [f64; 4])>,
    pub(super) draft_slice: Option<TableSlice>,
    pub(super) applied_slice: Option<TableSlice>,
    pub(super) slice_footer_height: f32,
}

impl HeatmapState {
    pub(super) fn new() -> Self {
        Self {
            slice_footer_height: 150.0,
            ..Self::default()
        }
    }
}

/// 1D profile state.
#[derive(Default)]
pub(super) struct ProfileState {
    pub(super) cache: Option<(ProfileKey, Result<ProfileData, String>)>,
    pub(super) job: ProgressJob<ProfileKey, ProfileData, ProfileData>,
    pub(super) load_generation: Arc<AtomicU64>,
    pub(super) draft_slice: Option<ProfileSlice>,
    pub(super) applied_slice: Option<ProfileSlice>,
    pub(super) footer_height: f32,
    pub(super) zoom: f32,
}

impl ProfileState {
    pub(super) fn new() -> Self {
        Self {
            footer_height: 170.0,
            zoom: 1.0,
            ..Self::default()
        }
    }
}

/// 3D volume data, raster jobs, and camera controls.
pub(super) struct VolumeState {
    pub(super) cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    pub(super) overview_cache: Option<(VolumeKey, Result<Arc<ExactVolumeData>, String>)>,
    pub(super) refined_cache: Vec<(VolumeKey, Arc<ExactVolumeData>)>,
    pub(super) job: BackgroundJob<VolumeKey, Arc<ExactVolumeData>>,
    pub(super) refine_generation: Arc<AtomicU64>,
    pub(super) raster_generation: Arc<AtomicU64>,
    pub(super) navigation_at: Option<Instant>,
    pub(super) raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    pub(super) overview_raster: Option<(VolumeRasterKey, RegisteredVolumeRaster)>,
    pub(super) raster_job: Option<VolumeRasterJob>,
    pub(super) draft_slice: Option<VolumeSlice>,
    pub(super) applied_slice: Option<VolumeSlice>,
    pub(super) draft_opacity: f32,
    pub(super) opacity: f32,
    pub(super) draft_log_scale: bool,
    pub(super) log_scale: bool,
    pub(super) projection: Option<VolumeProjection>,
    pub(super) axis_corner: [f32; 3],
    pub(super) rotation: VolumeRotation,
    pub(super) zoom: f32,
    pub(super) pan: [f32; 2],
    pub(super) range_drag: Option<(usize, bool)>,
    pub(super) footer_height: f32,
    pub(super) visibility: ValueVisibility,
}

impl VolumeState {
    pub(super) fn new(settings: &Settings) -> Self {
        Self {
            cache: None,
            overview_cache: None,
            refined_cache: Vec::new(),
            job: BackgroundJob::default(),
            refine_generation: Arc::new(AtomicU64::new(0)),
            raster_generation: Arc::new(AtomicU64::new(0)),
            navigation_at: None,
            raster: None,
            overview_raster: None,
            raster_job: None,
            draft_slice: None,
            applied_slice: None,
            draft_opacity: settings.volume_default_opacity,
            opacity: settings.volume_default_opacity,
            draft_log_scale: settings.volume_default_log_scale,
            log_scale: settings.volume_default_log_scale,
            projection: Some(VolumeProjection::Isometric),
            axis_corner: [0.5; 3],
            rotation: VolumeRotation::from_angles(0.65, 0.45),
            zoom: 1.0,
            pan: [0.0, 0.0],
            range_drag: None,
            footer_height: 260.0,
            visibility: ValueVisibility {
                fill: false,
                ..ValueVisibility::default()
            },
        }
    }
}

/// Map and globe caches, controls, projection, and globe camera.
pub(super) struct GeoState {
    pub(super) map_cache: Option<(MapKey, Result<MapData, String>)>,
    pub(super) map_job: ProgressJob<MapKey, (MapData, Option<Arc<Region>>), MapData>,
    pub(super) map_plane: Option<(MapPlaneKey, Arc<Region>)>,
    pub(super) map_load_generation: Arc<AtomicU64>,
    pub(super) map_controls: GeoViewControls,
    pub(super) globe_controls: GeoViewControls,
    pub(super) selected_pics: usize,
    pub(super) selected_site_group: Option<String>,
    pub(super) map_viewport: MapViewport,
    pub(super) map_footer_height: f32,
    pub(super) globe_footer_height: f32,
    pub(super) earth_texture: Option<EarthTexture>,
    pub(super) globe_yaw: f32,
    pub(super) globe_pitch: f32,
    pub(super) globe_zoom: f32,
    pub(super) globe_auto_center_source: Option<(usize, String)>,
    pub(super) globe_auto_center_pending: bool,
    pub(super) globe_rotation: Option<GlobeRotation>,
}

impl GeoState {
    pub(super) fn new(settings: &Settings) -> Self {
        Self {
            map_cache: None,
            map_job: ProgressJob::default(),
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
        }
    }
}
