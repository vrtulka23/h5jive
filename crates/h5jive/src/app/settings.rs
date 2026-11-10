use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WorkspaceMode {
    DatasetBrowser,
    DataInspector,
    Comparison,
}
impl WorkspaceMode {
    pub(super) const ALL: [Self; 3] = [Self::DatasetBrowser, Self::DataInspector, Self::Comparison];
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::DatasetBrowser => "Dataset browser",
            Self::DataInspector => "Data inspector",
            Self::Comparison => "Comparison",
        }
    }
}

pub(super) fn parse_graphics_backend(key: &str) -> Option<wgpu::Backends> {
    match key {
        "auto" => None,
        "metal" => Some(wgpu::Backends::METAL),
        "vulkan" => Some(wgpu::Backends::VULKAN),
        "gl" => Some(wgpu::Backends::GL),
        "vulkan,gl" => Some(wgpu::Backends::VULKAN | wgpu::Backends::GL),
        _ => unreachable!("DIPL validates viewer.graphics backend options"),
    }
}

pub(super) fn default_wgpu_backends(forwarded_x11: wgpu::Backends) -> wgpu::Backends {
    #[cfg(target_os = "linux")]
    {
        // SSH forwards X11 windows, but Vulkan often cannot present to them.
        let is_forwarded_x11 = env::var_os("WAYLAND_DISPLAY").is_none()
            && env::var("DISPLAY").is_ok_and(|display| {
                env::var_os("SSH_CONNECTION").is_some()
                    || display.starts_with("localhost:")
                    || display.starts_with("127.0.0.1:")
                    || display.starts_with("[::1]:")
            });
        if is_forwarded_x11 {
            return forwarded_x11;
        }
        return wgpu::Backends::VULKAN | wgpu::Backends::GL;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = forwarded_x11;
        wgpu::Backends::PRIMARY
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ViewMode {
    Spatial3d,
    Slice2d,
    Profiles1d,
    WorldMap,
    Globe,
    Data,
}
impl ViewMode {
    pub(super) const ALL: [Self; 6] = [
        Self::Spatial3d,
        Self::Slice2d,
        Self::Profiles1d,
        Self::WorldMap,
        Self::Globe,
        Self::Data,
    ];
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Spatial3d => "spatial_3d",
            Self::Slice2d => "slice_2d",
            Self::Profiles1d => "profiles_1d",
            Self::WorldMap => "world_map",
            Self::Globe => "globe",
            Self::Data => "data",
        }
    }
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Spatial3d => "3D projection",
            Self::Slice2d => "2D slice",
            Self::Profiles1d => "1D profiles",
            Self::WorldMap => "World map",
            Self::Globe => "Globe",
            Self::Data => "Data",
        }
    }
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Spatial3d => "Interactive voxel preview",
            Self::Slice2d => "Heat map view",
            Self::Profiles1d => "Line profile view",
            Self::WorldMap => "Flat world map view",
            Self::Globe => "Rotatable geospatial globe",
            Self::Data => "Dataset values and metadata",
        }
    }
    pub(super) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }
}

#[derive(Clone)]
pub(super) struct Palette {
    pub(super) background: [f32; 4],
    pub(super) panel: [f32; 4],
    pub(super) border: [f32; 4],
    pub(super) table_row_alt: [f32; 4],
    pub(super) muted: [f32; 4],
    pub(super) text: [f32; 4],
    pub(super) accent: [f32; 4],
    pub(super) splitter_idle: [f32; 4],
    pub(super) splitter_active: [f32; 4],
    pub(super) heatmap_low: [f32; 4],
    pub(super) heatmap_mid: [f32; 4],
    pub(super) heatmap_high: [f32; 4],
    pub(super) nan: [f32; 4],
    pub(super) positive_inf: [f32; 4],
    pub(super) negative_inf: [f32; 4],
    pub(super) fill: [f32; 4],
    pub(super) view_idle: [f32; 4],
    pub(super) view_hover: [f32; 4],
    pub(super) view_selected: [f32; 4],
    pub(super) view_stroke: [f32; 4],
    pub(super) view_border_idle: [f32; 4],
    pub(super) view_border_selected: [f32; 4],
    pub(super) box_alpha: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColormapPreset {
    Theme,
    Viridis,
    Grayscale,
}
impl ColormapPreset {
    pub(super) fn from_key(key: &str) -> Option<Self> {
        match key {
            "theme" => Some(Self::Theme),
            "viridis" => Some(Self::Viridis),
            "grayscale" => Some(Self::Grayscale),
            _ => None,
        }
    }
    pub(super) fn apply(self, palette: &mut Palette) {
        let rgb = |r: u8, g: u8, b: u8| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
        let stops = match self {
            Self::Theme => return,
            Self::Viridis => [rgb(68, 1, 84), rgb(33, 145, 140), rgb(253, 231, 37)],
            Self::Grayscale => [rgb(32, 32, 32), rgb(136, 136, 136), rgb(240, 240, 240)],
        };
        palette.heatmap_low = stops[0];
        palette.heatmap_mid = stops[1];
        palette.heatmap_high = stops[2];
    }
}
#[derive(Clone)]
pub(super) struct ThemeProfile {
    pub(super) name: String,
    pub(super) base: Theme,
    pub(super) rounding: f32,
    pub(super) palette: Palette,
}
pub(super) struct ThemeColors<'ui>(pub(super) Vec<ColorStackToken<'ui>>);

#[derive(Clone, Debug)]
pub(super) struct PicsRegion {
    pub(super) name: String,
    pub(super) bounds: [f32; 4], // west, south, east, north
}
#[derive(Clone, Debug)]
pub(super) struct PicsSite {
    pub(super) name: String,
    pub(super) country: String,
    pub(super) category: String,
    pub(super) center: [f32; 2], // longitude, latitude
    pub(super) reference: bool,
    pub(super) regions: Vec<PicsRegion>,
    pub(super) usable_km: Option<[i64; 2]>,
    pub(super) altitude_m: Option<i64>,
    pub(super) landsat_wrs_2: Option<String>,
    pub(super) status: String,
    pub(super) notes: String,
    pub(super) source_url: String,
}

fn read_pics_sites(dip: &Project) -> Result<Vec<PicsSite>, Box<dyn std::error::Error>> {
    let mut sites = Vec::new();
    for (id, _) in dip.node("pics")?.entries()? {
        let key = |field: &str| format!("pics[{id}].{field}");
        let number = |field: &str| dip.node(&key(field))?.as_f64().map(|value| value as f32);
        let reference = dip.node(&key("ceos_west")).is_ok();
        let general = dip.node(&key("category")).is_ok();
        let region = |prefix: &str, name: &str| -> Result<PicsRegion, Box<dyn std::error::Error>> {
            Ok(PicsRegion {
                name: name.into(),
                bounds: [
                    number(&format!("{prefix}_west"))?,
                    number(&format!("{prefix}_south"))?,
                    number(&format!("{prefix}_east"))?,
                    number(&format!("{prefix}_north"))?,
                ],
            })
        };
        let regions = if general {
            vec![region("roi", &dip.node(&key("roi_name"))?.as_string()?)?]
        } else if reference {
            vec![region("ceos", "CEOS")?, region("cnes", "CNES")?]
        } else {
            let mut regions = vec![region(
                "roi_1",
                &dip.node(&key("roi_1_name"))?.as_string()?,
            )?];
            if let Ok(name) = dip.node(&key("roi_2_name")) {
                regions.push(region("roi_2", &name.as_string()?)?);
            }
            regions
        };
        sites.push(PicsSite {
            name: dip.node(&key("name"))?.as_string()?,
            country: dip.node(&key("country"))?.as_string()?,
            category: if general {
                dip.node(&key("category"))?.as_string()?
            } else {
                "Desert PICS".into()
            },
            center: [number("center_longitude")?, number("center_latitude")?],
            reference,
            regions,
            usable_km: if general {
                None
            } else {
                Some([
                    dip.node(&key("usable_km_1"))?.as_i64()?,
                    dip.node(&key("usable_km_2"))?.as_i64()?,
                ])
            },
            altitude_m: (!general)
                .then(|| dip.node(&key("altitude_m"))?.as_i64())
                .transpose()?,
            landsat_wrs_2: (!general)
                .then(|| dip.node(&key("landsat_wrs_2"))?.as_string())
                .transpose()?,
            status: dip.node(&key("status"))?.as_string()?,
            notes: if general {
                dip.node(&key("notes"))?.as_string()?
            } else {
                String::new()
            },
            source_url: dip.node(&key("source_url"))?.as_string()?,
        });
    }
    sites.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sites)
}

fn read_dataset_rules(dip: &Project) -> Result<Vec<GeolocationRule>, Box<dyn std::error::Error>> {
    let mut rules = Vec::new();
    for (id, _) in dip.node("dataset_formats")?.entries()? {
        let field = |name: &str| {
            dip.node(&format!("dataset_formats[{id}].{name}"))?
                .as_string()
        };
        let scan_variable = field("scan_direction.variable")?;
        let scan_direction = if scan_variable.is_empty() {
            None
        } else {
            let values = |name: &str| -> Result<Vec<i64>, Box<dyn std::error::Error>> {
                Ok(field(name)?
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::parse)
                    .collect::<Result<Vec<_>, _>>()?)
            };
            let forward_values = values("scan_direction.forward_values")?;
            let backward_values = values("scan_direction.backward_values")?;
            if forward_values
                .iter()
                .any(|value| backward_values.contains(value))
            {
                return Err(format!("Invalid scan direction flag values for {id}").into());
            }
            let group = field("scan_direction.group")?;
            Some(h5jive_data::ScanDirectionRule {
                group,
                variable: scan_variable,
                forward_values,
                backward_values,
                default_filter: match field("scan_direction.default_filter")?.as_str() {
                    "both" => ScanFilter::Both,
                    "forward" => ScanFilter::Forward,
                    "backward" => ScanFilter::Backward,
                    _ => unreachable!("DIPL validates scan_direction.default_filter"),
                },
            })
        };
        rules.push(GeolocationRule {
            file_glob: field("identification.file_glob")?,
            platform: field("identification.platform")?,
            sensor: field("identification.sensor")?,
            product_type: field("identification.product_type")?,
            observation_group: field("observation_group")?,
            coordinate_group: field("coordinate_group")?,
            latitude: field("latitude")?,
            longitude: field("longitude")?,
            latitude_bounds: field("latitude_bounds")?,
            longitude_bounds: field("longitude_bounds")?,
            scan_direction,
        });
    }
    Ok(rules)
}
impl Drop for ThemeColors<'_> {
    fn drop(&mut self) {
        while let Some(token) = self.0.pop() {
            drop(token);
        }
    }
}
pub(super) fn mix_color(a: [f32; 4], b: [f32; 4], fraction: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * fraction,
        a[1] + (b[1] - a[1]) * fraction,
        a[2] + (b[2] - a[2]) * fraction,
        1.0,
    ]
}
fn read_profile(dip: &Project, name: &str) -> Result<ThemeProfile, Box<dyn std::error::Error>> {
    let key = |field: &str| format!("themes[{name}].{field}");
    let base = match dip.node(&key("base"))?.as_string()?.as_str() {
        "dark" => Theme::Dark,
        "light" => Theme::Light,
        "classic" => Theme::Classic,
        _ => unreachable!("DIPL validates appearance_profile.base"),
    };
    let background = read_color(dip, &key("background"))?;
    let panel = read_color(dip, &key("panel"))?;
    let text = read_color(dip, &key("text"))?;
    let border = read_color(dip, &key("border"))?;
    let muted = read_color(dip, &key("muted"))?;
    let accent = read_color(dip, &key("accent"))?;
    Ok(ThemeProfile {
        name: name.to_owned(),
        base,
        rounding: dip.node(&key("rounding"))?.as_f64()? as f32,
        palette: Palette {
            background,
            panel,
            border,
            table_row_alt: read_color(dip, &key("table_row_alt"))?,
            muted,
            text,
            accent,
            splitter_idle: border,
            splitter_active: accent,
            heatmap_low: read_color(dip, &key("heatmap_low"))?,
            heatmap_mid: read_color(dip, &key("heatmap_mid"))?,
            heatmap_high: read_color(dip, &key("heatmap_high"))?,
            nan: read_color(dip, &key("nan"))?,
            positive_inf: read_color(dip, &key("positive_inf"))?,
            negative_inf: read_color(dip, &key("negative_inf"))?,
            fill: read_color(dip, &key("fill"))?,
            view_idle: panel,
            view_hover: mix_color(panel, accent, 0.25),
            view_selected: accent,
            view_stroke: text,
            view_border_idle: border,
            view_border_selected: accent,
            box_alpha: 0.65,
        },
    })
}
fn read_color(dip: &Project, path: &str) -> Result<[f32; 4], Box<dyn std::error::Error>> {
    let value = dip.node(path)?.as_string()?;
    let rgb = u32::from_str_radix(&value[1..], 16)?;
    Ok([
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        1.0,
    ])
}

struct ViewerValues<'a>(&'a Project);

impl ViewerValues<'_> {
    fn string(&self, path: &str) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.0.node(&format!("viewer.{path}"))?.as_string()?)
    }

    fn boolean(&self, path: &str) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.0.node(&format!("viewer.{path}"))?.as_bool()?)
    }

    fn integer(&self, path: &str) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(self.0.node(&format!("viewer.{path}"))?.as_i64()?)
    }

    fn count(&self, path: &str) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(self.integer(path)?.try_into()?)
    }

    fn pixels(&self, path: &str) -> Result<f32, Box<dyn std::error::Error>> {
        Ok(self.integer(path)? as f32)
    }

    fn decimal(&self, path: &str) -> Result<f64, Box<dyn std::error::Error>> {
        Ok(self.0.node(&format!("viewer.{path}"))?.as_f64()?)
    }

    fn fraction(&self, path: &str) -> Result<f32, Box<dyn std::error::Error>> {
        Ok(self.decimal(path)? as f32)
    }
}

pub(super) struct Settings {
    pub(super) dataset_rules: Vec<GeolocationRule>,
    pub(super) dataset_sources: Vec<DatasetSource>,
    pub(super) profiles: Vec<ThemeProfile>,
    pub(super) active_profile: usize,
    pub(super) default_view: ViewMode,
    pub(super) graphics_backend: Option<wgpu::Backends>,
    pub(super) forwarded_x11_backend: wgpu::Backends,
    pub(super) sidebar_width: f32,
    pub(super) sidebar_min_width: f32,
    pub(super) sidebar_max_width: f32,
    pub(super) viewer_min_width: f32,
    pub(super) details_panel_fraction: f32,
    pub(super) panel_min_height: f32,
    pub(super) splitter_width: f32,
    pub(super) view_button_height: f32,
    pub(super) view_button_gap: f32,
    pub(super) update_button_width: f32,
    pub(super) update_button_height: f32,
    pub(super) tree_max_depth: usize,
    pub(super) tree_max_objects: usize,
    pub(super) pinned_default: PinnedDefault,
    pub(super) display_decimals: usize,
    pub(super) region_cache_entries: usize,
    pub(super) statistics_sample_values: usize,
    pub(super) histogram_bins: usize,
    pub(super) details_max_width: f32,
    pub(super) table_columns_per_page: usize,
    pub(super) table_row_header_width: f32,
    pub(super) table_value_column_width: f32,
    pub(super) table_scrollbar_padding: f32,
    pub(super) heatmap_cell_size: f32,
    pub(super) heatmap_colormap_preset: ColormapPreset,
    pub(super) heatmap_auto_range: bool,
    pub(super) heatmap_value_min: f64,
    pub(super) heatmap_value_max: f64,
    pub(super) heatmap_legend_width: f32,
    pub(super) heatmap_legend_fraction: f32,
    pub(super) heatmap_max_cells: usize,
    pub(super) distribution_min_width: f32,
    pub(super) distribution_min_height: f32,
    pub(super) distribution_x_tick_spacing: f32,
    pub(super) distribution_y_tick_intervals: usize,
    pub(super) volume_default_opacity: f32,
    pub(super) volume_default_log_scale: bool,
    pub(super) volume_max_zoom: f32,
    pub(super) volume_back_brightness: f32,
    pub(super) map_max_points: usize,
    pub(super) map_point_radius: f32,
    pub(super) map_background: String,
    pub(super) globe_background: String,
    pub(super) map_show_pics: bool,
    pub(super) map_pinned_default: PinnedDefault,
    pub(super) globe_pinned_default: PinnedDefault,
    pub(super) map_max_zoom: f32,
    pub(super) map_site_zoom: f32,
    pub(super) map_zoom_factor: f32,
    pub(super) globe_zoom_min: f32,
    pub(super) globe_zoom_max: f32,
    pub(super) globe_zoom_factor: f32,
    pub(super) pics_sites: Vec<PicsSite>,
    pub(super) show_dataset_shape: bool,
    pub(super) title: String,
    pub(super) palette: Palette,
}
impl Settings {
    pub(super) fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let project = std::path::Path::new("settings/DIPfile");
        // An extracted release keeps its editable settings beside the executable.
        let executable = std::env::current_exe()?;
        // Windows canonicalize() and current_exe() add a \\?\ verbatim prefix
        // that the SNT3 C++ loader cannot reopen after its MinGW std::filesystem::absolute
        // misparses it as a relative path. dunce strips this prefix safely on Windows.
        let executable = dunce::canonicalize(&executable).unwrap_or(executable);
        let bundled = executable
            .parent()
            .map(|directory| directory.join("settings/DIPfile"));
        let project = if project.exists() {
            project.to_path_buf()
        } else if let Some(bundled) = bundled.filter(|path| path.exists()) {
            bundled
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../settings/DIPfile")
        };
        let project = dunce::simplified(&project);
        let dip = Project::load(project)?;
        let values = ViewerValues(&dip);
        let default_view = values.string("application.default_view")?;
        let default_view = ViewMode::from_key(&default_view)
            .expect("DIPL validates viewer.application.default_view");
        let graphics_backend = parse_graphics_backend(&values.string("graphics.backend")?);
        let forwarded_x11_backend =
            parse_graphics_backend(&values.string("graphics.forwarded_x11_backend")?)
                .expect("DIPL validates viewer.graphics.forwarded_x11_backend");
        let pinned_default = values.string("data.pinned_default")?;
        let pinned_default = PinnedDefault::from_key(&pinned_default)
            .expect("DIPL validates viewer.data.pinned_default");
        let heatmap_value_min = values.decimal("views.slice_2d.value_min")?;
        let heatmap_value_max = values.decimal("views.slice_2d.value_max")?;
        let heatmap_auto_range = match values.string("views.slice_2d.range_mode")?.as_str() {
            "auto" => true,
            "fixed" => false,
            _ => unreachable!("DIPL validates viewer.views.slice_2d.range_mode"),
        };
        let sidebar_min_width = values.pixels("layout.sidebar_min_width")?;
        let sidebar_max_width = values.pixels("layout.sidebar_max_width")?;
        let sidebar_width = values.pixels("layout.sidebar_width")?;
        let colormap_key = values.string("views.slice_2d.colormap")?;
        let colormap = ColormapPreset::from_key(&colormap_key)
            .expect("DIPL validates viewer.views.slice_2d.colormap");
        let mut profiles = dip
            .node("themes")?
            .entries()?
            .into_iter()
            .map(|(name, _)| read_profile(&dip, &name))
            .collect::<Result<Vec<_>, _>>()?;
        for profile in &mut profiles {
            colormap.apply(&mut profile.palette);
        }
        let profile_name = values.string("appearance.profile")?;
        let active_profile = profiles
            .iter()
            .position(|profile| profile.name == profile_name)
            .ok_or_else(|| format!("Appearance profile {profile_name} is not loaded"))?;
        let palette = profiles[active_profile].palette.clone();
        let settings = Self {
            dataset_rules: read_dataset_rules(&dip)?,
            dataset_sources: read_dataset_sources(&dip, project.parent().unwrap())?,
            profiles,
            active_profile,
            default_view,
            graphics_backend,
            forwarded_x11_backend,
            sidebar_width,
            sidebar_min_width,
            sidebar_max_width,
            viewer_min_width: values.pixels("layout.viewer_min_width")?,
            details_panel_fraction: values.fraction("layout.details_fraction")?,
            panel_min_height: values.pixels("layout.panel_min_height")?,
            splitter_width: values.pixels("layout.splitter_width")?,
            view_button_height: values.pixels("layout.view_button_height")?,
            view_button_gap: values.pixels("layout.view_button_gap")?,
            update_button_width: values.pixels("layout.refresh_button_width")?,
            update_button_height: values.pixels("layout.refresh_button_height")?,
            tree_max_depth: values.count("tree.max_depth")?,
            tree_max_objects: values.count("tree.max_objects")?,
            pinned_default,
            display_decimals: values.count("format.decimals")?,
            region_cache_entries: values.count("data.region_cache_entries")?,
            statistics_sample_values: values.count("statistics.sample_values")?,
            histogram_bins: values.count("statistics.histogram_bins")?,
            details_max_width: values.pixels("layout.details_max_width")?,
            table_columns_per_page: values.count("views.data.table.columns_per_page")?,
            table_row_header_width: values.pixels("views.data.table.row_header_width")?,
            table_value_column_width: values.pixels("views.data.table.value_column_width")?,
            table_scrollbar_padding: values.pixels("views.data.table.scrollbar_padding")?,
            heatmap_cell_size: values.pixels("views.slice_2d.cell_size")?,
            heatmap_colormap_preset: colormap,
            heatmap_auto_range,
            heatmap_value_min,
            heatmap_value_max,
            heatmap_legend_width: values.pixels("views.slice_2d.legend_width")?,
            heatmap_legend_fraction: values.fraction("views.slice_2d.legend_fraction")?,
            heatmap_max_cells: values.count("views.slice_2d.max_cells")?,
            distribution_min_width: values.pixels("views.profiles_1d.min_width")?,
            distribution_min_height: values.pixels("views.profiles_1d.min_height")?,
            distribution_x_tick_spacing: values.pixels("views.profiles_1d.x_tick_spacing")?,
            distribution_y_tick_intervals: values.count("views.profiles_1d.y_tick_intervals")?,
            volume_default_opacity: values.fraction("views.spatial_3d.default_opacity")?,
            volume_default_log_scale: values.boolean("views.spatial_3d.default_log_scale")?,
            volume_max_zoom: values.fraction("views.spatial_3d.max_zoom")?,
            volume_back_brightness: values.fraction("views.spatial_3d.back_brightness")?,
            map_max_points: values.count("views.world_map.max_points")?,
            map_point_radius: values.fraction("views.world_map.point_radius")?,
            map_background: values.string("views.world_map.background")?,
            globe_background: values.string("views.globe.background")?,
            map_show_pics: values.boolean("views.world_map.show_pics")?,
            map_pinned_default: PinnedDefault::from_key(
                &values.string("views.world_map.pinned_default")?,
            )
            .expect("DIPL validates world map pinned_default"),
            globe_pinned_default: PinnedDefault::from_key(
                &values.string("views.globe.pinned_default")?,
            )
            .expect("DIPL validates globe pinned_default"),
            map_max_zoom: values.fraction("views.world_map.max_zoom")?,
            map_site_zoom: values.fraction("views.world_map.site_zoom")?,
            map_zoom_factor: values.fraction("views.world_map.zoom_factor")?,
            globe_zoom_min: values.fraction("views.globe.zoom_min")?,
            globe_zoom_max: values.fraction("views.globe.zoom_max")?,
            globe_zoom_factor: values.fraction("views.globe.zoom_factor")?,
            pics_sites: read_pics_sites(&dip)?,
            show_dataset_shape: values.boolean("tree.show_dataset_shape")?,
            title: values.string("application.title")?,
            palette,
        };
        Ok(settings)
    }
}
