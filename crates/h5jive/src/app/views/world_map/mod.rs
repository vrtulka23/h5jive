use super::super::*;
use super::{draw_geo_controls, observation_color, pixel_hover_distance};
use std::sync::OnceLock;

#[cfg(test)]
mod data;
mod pics;
use pics::draw_pics_overlay;

pub(crate) struct GeoSource {
    pub(super) selection: Selection,
    pub(super) shape: Vec<usize>,
    pub(super) geolocation: Geolocation,
}

pub(crate) fn coastlines() -> &'static Vec<Vec<[f32; 2]>> {
    static LINES: OnceLock<Vec<Vec<[f32; 2]>>> = OnceLock::new();
    LINES.get_or_init(|| {
        let mut paths = Vec::new();
        let mut path = Vec::new();
        for line in include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/coastline_110m.txt"
        ))
        .lines()
        {
            if line.is_empty() {
                if path.len() > 1 {
                    paths.push(std::mem::take(&mut path));
                }
            } else if !line.starts_with('#') {
                let mut parts = line.split_whitespace();
                if let (Some(lon), Some(lat)) = (parts.next(), parts.next()) {
                    if let (Ok(lon), Ok(lat)) = (lon.parse(), lat.parse()) {
                        path.push([lon, lat]);
                    }
                }
            }
        }
        if path.len() > 1 {
            paths.push(path);
        }
        paths
    })
}

fn project(origin: [f32; 2], size: [f32; 2], view: MapViewport, lon: f32, lat: f32) -> [f32; 2] {
    view.project(origin, size, lon, lat)
}

fn visible_map_rect(
    origin: [f32; 2],
    size: [f32; 2],
    canvas: [f32; 2],
    canvas_size: [f32; 2],
    view: MapViewport,
) -> ([f32; 2], [f32; 2]) {
    let top_left = project(origin, size, view, -180.0, 90.0);
    let bottom_right = project(origin, size, view, 180.0, -90.0);
    (
        [top_left[0].max(canvas[0]), top_left[1].max(canvas[1])],
        [
            bottom_right[0].min(canvas[0] + canvas_size[0]),
            bottom_right[1].min(canvas[1] + canvas_size[1]),
        ],
    )
}

#[cfg(test)]
mod badge_position_tests {
    use super::*;

    #[test]
    fn badge_follows_visible_map_edges_and_returns_on_reset() {
        let origin = [100.0, 200.0];
        let size = [400.0, 200.0];
        let canvas = [80.0, 100.0];
        let canvas_size = [440.0, 400.0];
        let initial = visible_map_rect(origin, size, canvas, canvas_size, MapViewport::default());
        assert_eq!(initial, ([100.0, 200.0], [500.0, 400.0]));

        let zoomed = MapViewport {
            center: [0.0, 0.0],
            zoom: 2.0,
        };
        assert_eq!(
            visible_map_rect(origin, size, canvas, canvas_size, zoomed),
            ([80.0, 100.0], [520.0, 500.0])
        );
        let panned = MapViewport {
            center: [90.0, 0.0],
            zoom: 2.0,
        };
        assert_eq!(
            visible_map_rect(origin, size, canvas, canvas_size, panned),
            ([80.0, 100.0], [500.0, 500.0])
        );
        assert_eq!(
            visible_map_rect(origin, size, canvas, canvas_size, MapViewport::default()),
            initial
        );
    }
}

fn longitude_near(longitude: f32, reference: f32) -> f32 {
    reference + (longitude - reference + 180.0).rem_euclid(360.0) - 180.0
}

pub(super) fn geospatial_point_budget(
    base: usize,
    zoom_factor: f32,
    zoom: f32,
    total: usize,
) -> usize {
    let level = zoom.log2().floor().max(0.0) as i32;
    ((base as f64 * (zoom_factor as f64).powi(level)) as usize).min(total)
}

impl App {
    pub(super) fn visible_geo_data(
        &self,
        key: Option<&MapKey>,
    ) -> Option<&Result<MapData, String>> {
        let key = key?;
        let (cached, result) = self.map_cache.as_ref()?;
        if cached == key {
            return Some(result);
        }
        let loading_same_source = cached.file == key.file
            && cached.path == key.path
            && result.is_ok()
            && self.map_job.is_some();
        loading_same_source.then_some(result)
    }

    pub(crate) fn selected_geo_source(&mut self) -> Option<GeoSource> {
        let selection = self.selection.clone()?;
        let entry = find_entry(&self.files[selection.file].tree, &selection.path)?;
        let shape = match &entry.kind {
            Kind::Dataset {
                shape,
                numeric: true,
            } if matches!(shape.len(), 2..=4) && shape.iter().all(|&size| size > 0) => {
                shape.clone()
            }
            _ => return None,
        };
        let cache_key = (selection.file, selection.path.clone());
        if !self.tree_geolocation_cache.contains_key(&cache_key) {
            let result = self.files[selection.file]
                .geolocation_with_rules(&selection.path, &self.settings.dataset_rules);
            if let Err(error) = &result {
                self.show_notice(
                    NoticeKind::Error,
                    format!("Could not read dataset geolocation: {error}"),
                );
            }
            self.tree_geolocation_cache
                .insert(cache_key.clone(), result.ok().flatten());
        }
        let geolocation = self
            .tree_geolocation_cache
            .get(&cache_key)
            .cloned()
            .flatten()?;
        Some(GeoSource {
            selection,
            shape,
            geolocation,
        })
    }

    pub(super) fn ensure_geo_data(
        &mut self,
        source: Option<&GeoSource>,
        options: GeoViewOptions,
        max_points: usize,
    ) -> Option<MapKey> {
        let source = source?;
        let key = MapKey {
            file: source.selection.file,
            path: source.selection.path.clone(),
            time: options.time,
            spectral: options.pinned,
            max_points,
        };
        if let Some((running, receiver)) = self.map_job.take() {
            let mut completed = false;
            loop {
                match receiver.try_recv() {
                    Ok(MapJobUpdate::Preview(data)) => {
                        self.map_cache = Some((running.clone(), Ok(data)));
                    }
                    Ok(MapJobUpdate::Complete(result)) => {
                        match result {
                            Ok((data, plane)) => {
                                if let Some(plane) = plane {
                                    self.map_plane = Some((
                                        MapPlaneKey {
                                            file: running.file,
                                            path: running.path.clone(),
                                            time: running.time.unwrap_or(0),
                                            spectral: running.spectral,
                                        },
                                        plane,
                                    ));
                                }
                                self.map_cache = Some((running.clone(), Ok(data)));
                            }
                            Err(error) => {
                                self.notices
                                    .error(format!("Could not load map data: {error}"));
                                self.map_cache = Some((running.clone(), Err(error)));
                            }
                        }
                        completed = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let error = "Map worker stopped unexpectedly";
                        self.notices
                            .error(format!("Could not load map data: {error}"));
                        self.map_cache = Some((running.clone(), Err(error.into())));
                        completed = true;
                        break;
                    }
                }
            }
            if !completed {
                self.map_job = Some((running, receiver));
                return Some(key);
            }
        }
        if self
            .map_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return Some(key);
        }
        let file_path = self.file_paths[source.selection.file].clone();
        let path = source.selection.path.clone();
        let shape = source.shape.clone();
        let geolocation = source.geolocation.clone();
        let time = options.time;
        let spectral = options.pinned.min(shape[shape.len() - 1] - 1);
        let preview_points = max_points.min(self.settings.map_max_points);
        let plane_key = MapPlaneKey {
            file: key.file,
            path: key.path.clone(),
            time: time.unwrap_or(0),
            spectral: key.spectral,
        };
        let cached_plane = self
            .map_plane
            .as_ref()
            .filter(|(cached, _)| cached == &plane_key)
            .map(|(_, plane)| Arc::clone(plane));
        let generation = Arc::clone(&self.map_load_generation);
        let expected_generation = generation.load(Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = (|| {
                let cancelled = || generation.load(Ordering::Relaxed) != expected_generation;
                if cancelled() {
                    return Err("Map request cancelled".into());
                }
                let file = open_dataset_reader(&file_path)?;
                let (rows, columns) = if shape.len() >= 3 {
                    (shape[1], shape[2])
                } else {
                    (shape[0], shape[1])
                };
                let single_time = shape.len() == 2 || shape[0] == 1 || time.is_some();
                let cacheable = single_time
                    && rows
                        .checked_mul(columns)
                        .is_some_and(|count| count <= MAX_PLANE_CELLS);
                let mut plane = cached_plane;
                if cacheable {
                    let disk_key = MapPlaneCacheKey {
                        source: &file_path,
                        dataset: &path,
                        time: time.unwrap_or(0),
                        spectral,
                        rows,
                        columns,
                    };
                    if plane.is_none() {
                        plane = load_map_plane(&disk_key).map(Arc::new);
                    }
                }
                if plane.is_none() && single_time && rows.saturating_mul(columns) > 100_000 {
                    if let Ok(preview) = load_map_preview(
                        &file,
                        &path,
                        &shape,
                        &geolocation,
                        time,
                        spectral,
                        preview_points,
                    ) {
                        if sender.send(MapJobUpdate::Preview(preview)).is_err() {
                            return Err("Map request cancelled".into());
                        }
                    }
                }
                if cacheable && plane.is_none() {
                    let disk_key = MapPlaneCacheKey {
                        source: &file_path,
                        dataset: &path,
                        time: time.unwrap_or(0),
                        spectral,
                        rows,
                        columns,
                    };
                    let decoded = read_map_radiance_plane_with_cancel(
                        &file, &path, &shape, time, spectral, &cancelled,
                    )?;
                    if cancelled() {
                        return Err("Map request cancelled".into());
                    }
                    store_map_plane(&disk_key, &decoded);
                    plane = Some(Arc::new(decoded));
                }
                if cancelled() {
                    return Err("Map request cancelled".into());
                }
                let data = if let Some(decoded) = plane.as_deref() {
                    load_map_data_with_radiance(
                        &file,
                        &path,
                        &shape,
                        &geolocation,
                        time,
                        spectral,
                        max_points,
                        decoded,
                    )?
                } else {
                    load_map_data(
                        &file,
                        &path,
                        &shape,
                        &geolocation,
                        time,
                        spectral,
                        max_points,
                    )?
                };
                Ok((data, plane))
            })();
            let _ = sender.send(MapJobUpdate::Complete(result));
        });
        self.map_job = Some((key.clone(), receiver));
        Some(key)
    }

    pub(crate) fn draw_world_map_view(&mut self, ui: &Ui, plot_ctx: &PlotContext) {
        let source = self.selected_geo_source();
        self.map_controls.sync_source(
            source
                .as_ref()
                .map(|source| (&source.selection, source.shape.as_slice())),
            self.settings.map_pinned_default,
            source
                .as_ref()
                .and_then(|source| source.geolocation.scan_direction.as_ref())
                .map_or(ScanFilter::Both, |scan| scan.default_filter),
        );
        let options = self.map_controls.applied;
        let has_pixel_bounds = source.as_ref().is_some_and(|source| {
            source.geolocation.latitude_bounds.is_some()
                && source.geolocation.longitude_bounds.is_some()
        });
        let max_points = if let Some(source) = source.as_ref() {
            geospatial_point_budget(
                self.settings.map_max_points,
                self.settings.map_zoom_detail_factor,
                self.map_viewport.zoom,
                source.shape.iter().take(3).product(),
            )
        } else {
            self.settings.map_max_points
        };
        let key = self.ensure_geo_data(source.as_ref(), options, max_points);
        let available = ui.content_region_avail();
        let legend_width = self.settings.heatmap_legend_width.min(available[0] * 0.23);
        let legend_gap = PANEL_GAP;
        let controls_height = if self.map_footer_height > 0.0 {
            self.map_footer_height
        } else {
            geo_controls_height(
                ui,
                &self.settings,
                source.as_ref().map_or(0, |source| source.shape.len()),
                source
                    .as_ref()
                    .is_some_and(|source| source.geolocation.scan_direction.is_some()),
            )
        };
        let content_start_y = ui.cursor_pos_y();
        let map_area = [
            (available[0] - legend_width - legend_gap).max(1.0),
            (available[1] - controls_height).max(1.0),
        ];
        let map_width = map_area[0].min(map_area[1] * 2.0);
        let map_size = [map_width, map_width * 0.5];
        let cursor = ui.cursor_screen_pos();
        self.view_rect = [cursor[0], cursor[1], available[0], map_area[1]];
        let origin = [
            cursor[0] + (map_area[0] - map_width) * 0.5,
            cursor[1] + (map_area[1] - map_size[1]) * 0.5,
        ];
        ui.invisible_button("##map_canvas", map_area);
        let mouse = ui.io().mouse_pos();
        let inside = mouse[0] >= cursor[0]
            && mouse[0] <= cursor[0] + map_area[0]
            && mouse[1] >= cursor[1]
            && mouse[1] <= cursor[1] + map_area[1];
        let hovered = ui.is_item_hovered() && inside;
        if hovered && ui.io().mouse_wheel() != 0.0 {
            let factor = self.settings.map_zoom_factor.powf(ui.io().mouse_wheel());
            self.map_viewport
                .zoom_at(origin, map_size, mouse, factor, self.settings.map_max_zoom);
        }
        if ui.is_item_active() && ui.is_mouse_dragging(dear_app::imgui::MouseButton::Left) {
            self.map_viewport.pan(map_size, ui.io().mouse_delta());
        }
        let view = self.map_viewport;
        let draw = ui.get_window_draw_list();
        let palette = &self.settings.palette;
        let ocean = palette.panel;
        draw.add_rect(
            cursor,
            [cursor[0] + map_area[0], cursor[1] + map_area[1]],
            ocean,
        )
        .filled(true)
        .build();
        let _clip = draw.push_clip_rect(
            cursor,
            [cursor[0] + map_area[0], cursor[1] + map_area[1]],
            true,
        );
        if let Some(texture) = self.earth_texture_id() {
            // Transform the whole image with the map. As zoom grows it can
            // expand into the padded canvas, which is clipped before controls.
            draw.add_image(
                texture,
                project(origin, map_size, view, -180.0, 90.0),
                project(origin, map_size, view, 180.0, -90.0),
                [0.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            );
        }
        let grid = mix_color(palette.border, palette.muted, 0.35);
        let grid_step = if view.zoom >= 32.0 {
            1
        } else if view.zoom >= 8.0 {
            5
        } else if view.zoom >= 3.0 {
            10
        } else {
            30
        };
        for lon in (-180..=180).step_by(grid_step) {
            let x = project(origin, map_size, view, lon as f32, 0.0)[0];
            draw.add_line([x, cursor[1]], [x, cursor[1] + map_area[1]], grid)
                .build();
        }
        for lat in (-90..=90).step_by(grid_step) {
            let y = project(origin, map_size, view, 0.0, lat as f32)[1];
            draw.add_line([cursor[0], y], [cursor[0] + map_area[0], y], grid)
                .build();
        }
        let shore = mix_color(palette.text, palette.muted, 0.45);
        for path in coastlines() {
            for segment in path.windows(2) {
                if (segment[0][0] - segment[1][0]).abs() > 180.0 {
                    continue;
                }
                draw.add_line(
                    project(origin, map_size, view, segment[0][0], segment[0][1]),
                    project(origin, map_size, view, segment[1][0], segment[1][1]),
                    shore,
                )
                .thickness(1.1)
                .build();
            }
        }
        draw.add_rect(
            cursor,
            [cursor[0] + map_area[0], cursor[1] + map_area[1]],
            palette.border,
        )
        .build();
        let mut nearest: Option<(&MapPoint, f32)> = None;
        let map_data = self.visible_geo_data(key.as_ref());
        if let Some(Ok(data)) = map_data {
            let range = if self.settings.heatmap_auto_range {
                data.range
            } else {
                Some((
                    self.settings.heatmap_value_min,
                    self.settings.heatmap_value_max,
                ))
            };
            for special_pass in [false, true] {
                for point in &data.points {
                    if !options.scan_filter.allows(point.scan_direction) {
                        continue;
                    }
                    if point.special.is_some() != special_pass {
                        continue;
                    }
                    if !self
                        .value_visibility
                        .allows_class(point.value, point.special)
                    {
                        continue;
                    }
                    let p = project(origin, map_size, view, point.longitude, point.latitude);
                    let center_visible = p[0] >= cursor[0] - 4.0
                        && p[0] <= cursor[0] + map_area[0] + 4.0
                        && p[1] >= cursor[1] - 4.0
                        && p[1] <= cursor[1] + map_area[1] + 4.0;
                    let polygon = if matches!(options.pixel_style, 1 | 2) {
                        point.footprint.map(|footprint| {
                            footprint.map(|corner| {
                                project(
                                    origin,
                                    map_size,
                                    view,
                                    longitude_near(corner[0], point.longitude),
                                    corner[1],
                                )
                            })
                        })
                    } else {
                        None
                    };
                    if let Some(polygon) = polygon.as_ref() {
                        let left = polygon.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                        let right = polygon
                            .iter()
                            .map(|p| p[0])
                            .fold(f32::NEG_INFINITY, f32::max);
                        let top = polygon.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                        let bottom = polygon
                            .iter()
                            .map(|p| p[1])
                            .fold(f32::NEG_INFINITY, f32::max);
                        let outside = right < cursor[0]
                            || left > cursor[0] + map_area[0]
                            || bottom < cursor[1]
                            || top > cursor[1] + map_area[1];
                        if outside {
                            continue;
                        }
                    } else if !center_visible {
                        continue;
                    }
                    let mut color = observation_color(
                        point,
                        range,
                        options.dummy_color,
                        options.dummy_rgb,
                        plot_ctx,
                        &self.settings,
                        palette,
                    );
                    color[3] *= options.opacity;
                    match options.pixel_style {
                        1 | 2 if polygon.is_some() => {
                            let polygon = polygon.unwrap();
                            if options.pixel_style == 2 {
                                draw.add_quad_filled(
                                    polygon[0], polygon[1], polygon[2], polygon[3], color,
                                );
                            } else {
                                draw.add_quad(
                                    polygon[0],
                                    polygon[1],
                                    polygon[2],
                                    polygon[3],
                                    color,
                                    1.0,
                                    dear_app::imgui::DrawFlags::NONE,
                                )
                            }
                        }
                        1 | 2 => draw
                            .add_circle(p, self.settings.map_point_radius, color)
                            .filled(true)
                            .build(),
                        _ => draw
                            .add_circle(p, self.settings.map_point_radius, color)
                            .filled(true)
                            .build(),
                    }
                    if hovered {
                        if let Some(distance) =
                            pixel_hover_distance(mouse, p, polygon.as_ref(), options.pixel_style)
                        {
                            if nearest.is_none_or(|(_, previous)| distance <= previous) {
                                nearest = Some((point, distance));
                            }
                        }
                    }
                }
            }
        }
        if let Some(Ok(data)) = map_data {
            let current = self
                .map_cache
                .as_ref()
                .is_some_and(|(cached, _)| key.as_ref() == Some(cached));
            let (status, detail) = geo_render_badge(data, current);
            let (badge_min, badge_max) = visible_map_rect(origin, map_size, cursor, map_area, view);
            draw_render_badge(ui, &draw, badge_min, badge_max, status, &detail, palette);
        }
        drop(_clip);
        drop(draw);
        let hovered_pics = if options.show_pics {
            draw_pics_overlay(
                ui,
                origin,
                map_size,
                cursor,
                map_area,
                view,
                &self.settings.pics_sites,
                self.selected_site_group.as_deref(),
                self.selected_pics,
                palette,
            )
        } else {
            false
        };
        if let Some(Ok(data)) = map_data {
            let range = if self.settings.heatmap_auto_range {
                data.range
            } else {
                Some((
                    self.settings.heatmap_value_min,
                    self.settings.heatmap_value_max,
                ))
            };
            draw_heatmap_legend(
                ui,
                plot_ctx,
                [
                    cursor[0] + map_area[0] + legend_gap,
                    cursor[1] + PLOT_NAV_HEIGHT,
                ],
                [legend_width, (map_area[1] - PLOT_NAV_HEIGHT).max(1.0)],
                range,
                false,
                self.settings.heatmap_colormap_preset,
                palette,
                self.settings.display_decimals,
            );
            if let Some((point, _)) = nearest.filter(|_| !hovered_pics) {
                show_plain_tip(
                    ui,
                    &format!(
                        "Lat {:.3}°, Lon {:.3}°\n{}: {}",
                        point.latitude,
                        point.longitude,
                        if data.aggregated {
                            "Mean value"
                        } else {
                            "Value"
                        },
                        point.special.map_or_else(
                            || format_value(point.value, self.settings.display_decimals),
                            |kind| kind.label().into()
                        )
                    ),
                );
            }
        }
        match map_data {
            Some(Ok(_)) => {}
            Some(Err(error)) => ui.text(format!("Map unavailable: {error}")),
            None if source.is_none() => {
                ui.text("Select a geolocated dataset to show observations.")
            }
            None => {}
        }
        let nav_width = if legend_width > 0.0 {
            legend_width
        } else {
            110.0_f32.min(available[0])
        };
        let controls_cursor = ui.cursor_screen_pos();
        let (nav, visibility_changed) = draw_visibility_navigation(
            ui,
            "world",
            [cursor[0] + available[0] - nav_width, cursor[1]],
            nav_width,
            self.map_viewport.zoom,
            &mut self.value_visibility,
            palette,
            self.settings.tutorial_tips,
            None,
        );
        if visibility_changed {
            self.profile_cache = None;
        }
        match nav {
            PlotNavAction::Reset => self.map_viewport = MapViewport::default(),
            PlotNavAction::ZoomOut | PlotNavAction::ZoomIn => {
                let step = if nav == PlotNavAction::ZoomIn { 1 } else { -1 };
                let center = [origin[0] + map_size[0] * 0.5, origin[1] + map_size[1] * 0.5];
                self.map_viewport.zoom_at(
                    origin,
                    map_size,
                    center,
                    self.settings.map_zoom_factor.powi(step),
                    self.settings.map_max_zoom,
                );
            }
            PlotNavAction::None | PlotNavAction::Center => {}
        }
        ui.set_cursor_screen_pos(controls_cursor);
        let changes = draw_geo_controls(
            ui,
            "map",
            &mut self.map_controls,
            source.as_ref().map(|source| source.shape.as_slice()),
            self.axis_names.get(3).map_or("axis 3", String::as_str),
            has_pixel_bounds,
            source
                .as_ref()
                .is_some_and(|source| source.geolocation.scan_direction.is_some()),
            &mut self.selected_pics,
            &mut self.selected_site_group,
            &self.settings,
        );
        if changes.site_changed {
            if let Some(site) = self
                .selected_pics
                .checked_sub(1)
                .and_then(|index| self.settings.pics_sites.get(index))
            {
                self.map_viewport.center = site.center;
                self.map_viewport.zoom = self.settings.map_site_zoom;
                self.map_viewport.clamp();
            } else {
                self.map_viewport = MapViewport::default();
            }
        }
        if changes.refresh_data {
            self.map_job = None;
            self.map_load_generation.fetch_add(1, Ordering::Relaxed);
        }
        // Use the measured footer on the next frame so controls meet the bottom edge.
        self.map_footer_height = (ui.cursor_pos_y() - content_start_y - map_area[1]).max(1.0);
    }
}
