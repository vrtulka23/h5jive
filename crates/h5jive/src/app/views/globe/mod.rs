use super::super::*;
use super::world_map::geospatial_point_budget;
use super::{coastlines, draw_geo_controls, observation_color, pixel_hover_distance};

#[derive(Clone, Copy)]
struct Projected {
    point: [f32; 2],
    depth: f32,
}

#[derive(Clone, Copy)]
struct TextureVertex {
    projected: Projected,
    uv: [f32; 2],
}

fn draw_visible_texture_patch(
    draw: &dear_app::imgui::DrawListMut<'_>,
    texture: dear_app::imgui::TextureId,
    corners: [TextureVertex; 4],
) {
    let mut polygon = Vec::with_capacity(5);
    for index in 0..4 {
        let current = corners[index];
        let next = corners[(index + 1) % 4];
        let current_visible = current.projected.depth >= 0.0;
        let next_visible = next.projected.depth >= 0.0;
        if current_visible {
            polygon.push(current);
        }
        if current_visible != next_visible {
            let t = current.projected.depth / (current.projected.depth - next.projected.depth);
            polygon.push(TextureVertex {
                projected: Projected {
                    point: [
                        current.projected.point[0]
                            + (next.projected.point[0] - current.projected.point[0]) * t,
                        current.projected.point[1]
                            + (next.projected.point[1] - current.projected.point[1]) * t,
                    ],
                    depth: 0.0,
                },
                uv: [
                    current.uv[0] + (next.uv[0] - current.uv[0]) * t,
                    current.uv[1] + (next.uv[1] - current.uv[1]) * t,
                ],
            });
        }
    }
    if polygon.len() < 3 {
        return;
    }
    let first = polygon[0];
    if polygon.len() == 4 {
        let [a, b, c] = [polygon[1], polygon[2], polygon[3]];
        draw.add_image_quad(
            texture,
            first.projected.point,
            a.projected.point,
            b.projected.point,
            c.projected.point,
            first.uv,
            a.uv,
            b.uv,
            c.uv,
            [1.0, 1.0, 1.0, 1.0],
        );
    } else {
        for index in 1..polygon.len() - 1 {
            let [a, b] = [polygon[index], polygon[index + 1]];
            draw.add_image_quad(
                texture,
                first.projected.point,
                a.projected.point,
                b.projected.point,
                b.projected.point,
                first.uv,
                a.uv,
                b.uv,
                b.uv,
                [1.0, 1.0, 1.0, 1.0],
            );
        }
    }
}

fn project(lon: f32, lat: f32, center: [f32; 2], radius: f32, yaw: f32, pitch: f32) -> Projected {
    let lon = lon.to_radians();
    let lat = lat.to_radians();
    let (sl, cl) = (lon.sin(), lon.cos());
    let (sp, cp) = (lat.sin(), lat.cos());
    let x = cp * sl;
    let y = sp;
    let z = cp * cl;
    let (sy, cy) = yaw.sin_cos();
    let (sx, cx) = pitch.sin_cos();
    let x = cy * x + sy * z;
    let z = -sy * (cp * sl) + cy * z;
    let y2 = cx * y - sx * z;
    let z2 = sx * y + cx * z;
    Projected {
        point: [center[0] + x * radius, center[1] - y2 * radius],
        depth: z2,
    }
}

impl App {
    pub(crate) fn draw_globe_view(&mut self, ui: &Ui, plot_ctx: &PlotContext) {
        let source = self.selected_geo_source();
        self.globe_controls.sync_source(
            source
                .as_ref()
                .map(|source| (&source.selection, source.shape.as_slice())),
            self.settings.globe_pinned_default,
            source
                .as_ref()
                .and_then(|source| source.geolocation.scan_direction.as_ref())
                .map_or(ScanFilter::Both, |scan| scan.default_filter),
        );
        let options = self.globe_controls.applied;
        let has_bounds = source.as_ref().is_some_and(|s| {
            s.geolocation.latitude_bounds.is_some() && s.geolocation.longitude_bounds.is_some()
        });
        let max_points = if let Some(source) = source.as_ref() {
            geospatial_point_budget(
                self.settings.map_max_points,
                self.settings.map_zoom_detail_factor,
                self.globe_zoom,
                source.shape.iter().take(3).product(),
            )
        } else {
            self.settings.map_max_points
        };
        let key = self.ensure_geo_data(source.as_ref(), options, max_points);
        let available = ui.content_region_avail();
        let legend_width = if key.is_some() {
            self.settings.heatmap_legend_width.min(available[0] * 0.22)
        } else {
            0.0
        };
        let legend_gap = if key.is_some() { PANEL_GAP } else { 0.0 };
        let controls_height = if self.globe_footer_height > 0.0 {
            self.globe_footer_height
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
        let area = [
            (available[0] - legend_width - legend_gap).max(1.0),
            (available[1] - controls_height).max(1.0),
        ];
        let cursor = ui.cursor_screen_pos();
        self.view_rect = [cursor[0], cursor[1], available[0], area[1]];
        let center = [cursor[0] + area[0] * 0.5, cursor[1] + area[1] * 0.5];
        let radius = (area[0].min(area[1]) * 0.43 * self.globe_zoom).max(10.0);
        ui.invisible_button("##globe_canvas", area);
        let mouse = ui.io().mouse_pos();
        let within =
            (mouse[0] - center[0]).powi(2) + (mouse[1] - center[1]).powi(2) <= radius.powi(2);
        if ui.is_item_hovered() && within && ui.io().mouse_wheel() != 0.0 {
            self.globe_zoom = (self.globe_zoom
                * self.settings.globe_zoom_factor.powf(ui.io().mouse_wheel()))
            .clamp(self.settings.globe_zoom_min, self.settings.globe_zoom_max);
        }
        if ui.is_item_active() && ui.is_mouse_dragging(dear_app::imgui::MouseButton::Left) {
            let d = ui.io().mouse_delta();
            self.globe_yaw += d[0] / radius;
            self.globe_pitch = (self.globe_pitch + d[1] / radius).clamp(-1.5, 1.5);
        }
        let earth_texture = self.earth_texture_id(&self.settings.globe_background);
        let draw = ui.get_window_draw_list();
        let clip = draw.push_clip_rect(cursor, [cursor[0] + area[0], cursor[1] + area[1]], true);
        let palette = &self.settings.palette;
        let ocean = if self.settings.globe_background == "ocean" {
            mix_color(palette.panel, palette.heatmap_low, 0.30)
        } else {
            palette.panel
        };
        draw.add_circle(center, radius, ocean).filled(true).build();
        draw.add_circle(center, radius, palette.border).build();
        if let Some(texture) = earth_texture {
            for south in (-90..90).step_by(5) {
                let north = south + 5;
                for west in (-180..180).step_by(5) {
                    let east = west + 5;
                    let corners = [
                        project(
                            west as f32,
                            north as f32,
                            center,
                            radius,
                            self.globe_yaw,
                            self.globe_pitch,
                        ),
                        project(
                            east as f32,
                            north as f32,
                            center,
                            radius,
                            self.globe_yaw,
                            self.globe_pitch,
                        ),
                        project(
                            east as f32,
                            south as f32,
                            center,
                            radius,
                            self.globe_yaw,
                            self.globe_pitch,
                        ),
                        project(
                            west as f32,
                            south as f32,
                            center,
                            radius,
                            self.globe_yaw,
                            self.globe_pitch,
                        ),
                    ];
                    let u0 = (west as f32 + 180.0) / 360.0;
                    let u1 = (east as f32 + 180.0) / 360.0;
                    let v0 = (90.0 - north as f32) / 180.0;
                    let v1 = (90.0 - south as f32) / 180.0;
                    draw_visible_texture_patch(
                        &draw,
                        texture,
                        [
                            TextureVertex {
                                projected: corners[0],
                                uv: [u0, v0],
                            },
                            TextureVertex {
                                projected: corners[1],
                                uv: [u1, v0],
                            },
                            TextureVertex {
                                projected: corners[2],
                                uv: [u1, v1],
                            },
                            TextureVertex {
                                projected: corners[3],
                                uv: [u0, v1],
                            },
                        ],
                    );
                }
            }
        }
        // Graticule and bundled coastlines are depth-tested against the visible hemisphere.
        for lon in (-150..=180).step_by(30) {
            let mut last = None;
            for lat in (-90..=90).step_by(2) {
                let p = project(
                    lon as f32,
                    lat as f32,
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                if p.depth >= 0.0 {
                    if let Some(prev) = last {
                        draw.add_line(prev, p.point, mix_color(palette.border, ocean, 0.25))
                            .build();
                    }
                    last = Some(p.point);
                } else {
                    last = None;
                }
            }
        }
        for lat in (-60..=60).step_by(30) {
            let mut last = None;
            for lon in (-180..=180).step_by(2) {
                let p = project(
                    lon as f32,
                    lat as f32,
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                if p.depth >= 0.0 {
                    if let Some(prev) = last {
                        draw.add_line(prev, p.point, mix_color(palette.border, ocean, 0.25))
                            .build();
                    }
                    last = Some(p.point);
                } else {
                    last = None;
                }
            }
        }
        let shore = mix_color(palette.text, palette.muted, 0.35);
        for line in coastlines() {
            for seg in line.windows(2) {
                let a = project(
                    seg[0][0],
                    seg[0][1],
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                let b = project(
                    seg[1][0],
                    seg[1][1],
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                if a.depth >= 0.0 && b.depth >= 0.0 {
                    draw.add_line(a.point, b.point, shore)
                        .thickness(1.15)
                        .build();
                }
            }
        }
        let map_data = self.visible_geo_data(key.as_ref());
        let visible_range = map_data
            .and_then(|result| result.as_ref().ok())
            .and_then(|data| data.range);
        let range = if self.settings.heatmap_auto_range {
            visible_range
        } else {
            Some((
                self.settings.heatmap_value_min,
                self.settings.heatmap_value_max,
            ))
        };
        let mut nearest: Option<(&MapPoint, f32)> = None;
        if let Some(Ok(data)) = map_data {
            for point in &data.points {
                if !options.scan_filter.allows(point.scan_direction) {
                    continue;
                }
                if !self
                    .value_visibility
                    .allows_class(point.value, point.special)
                {
                    continue;
                }
                let p = project(
                    point.longitude,
                    point.latitude,
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                if p.depth < 0.0
                    || (p.point[0] < cursor[0] - 32.0
                        || p.point[0] > cursor[0] + area[0] + 32.0
                        || p.point[1] < cursor[1] - 32.0
                        || p.point[1] > cursor[1] + area[1] + 32.0)
                {
                    continue;
                }
                let mut col = observation_color(
                    point,
                    range,
                    options.dummy_color,
                    options.dummy_rgb,
                    plot_ctx,
                    &self.settings,
                    palette,
                );
                let brightness = 0.48 + 0.52 * p.depth.clamp(0.0, 1.0);
                for channel in &mut col[..3] {
                    *channel *= brightness;
                }
                col[3] *= options.opacity;
                let polygon = if options.pixel_style == 0 {
                    None
                } else {
                    point.footprint.and_then(|corners| {
                        let projected = corners.map(|corner| {
                            project(
                                corner[0],
                                corner[1],
                                center,
                                radius,
                                self.globe_yaw,
                                self.globe_pitch,
                            )
                        });
                        projected
                            .iter()
                            .all(|corner| corner.depth >= 0.0)
                            .then(|| projected.map(|corner| corner.point))
                    })
                };
                if options.pixel_style == 0 || point.footprint.is_none() {
                    draw.add_circle(p.point, self.settings.map_point_radius, col)
                        .filled(true)
                        .build();
                } else if let Some(corners) = polygon {
                    if options.pixel_style == 2 {
                        draw.add_quad_filled(corners[0], corners[1], corners[2], corners[3], col);
                    } else {
                        draw.add_quad(
                            corners[0],
                            corners[1],
                            corners[2],
                            corners[3],
                            col,
                            1.0,
                            dear_app::imgui::DrawFlags::NONE,
                        );
                    }
                } else {
                    continue;
                }
                if within {
                    if let Some(distance) =
                        pixel_hover_distance(mouse, p.point, polygon.as_ref(), options.pixel_style)
                    {
                        if nearest.is_none_or(|(_, old)| distance <= old) {
                            nearest = Some((point, distance));
                        }
                    }
                }
            }
        }
        let mut hovered_pics = false;
        if options.show_pics {
            for (i, site) in self.settings.pics_sites.iter().enumerate() {
                if self
                    .selected_site_group
                    .as_deref()
                    .is_some_and(|group| site.category != group)
                {
                    continue;
                }
                let p = project(
                    site.center[0],
                    site.center[1],
                    center,
                    radius,
                    self.globe_yaw,
                    self.globe_pitch,
                );
                if p.depth < 0.0 {
                    continue;
                }
                for region in &site.regions {
                    let [w, s, e, n] = region.bounds;
                    let corners = [[w, s], [e, s], [e, n], [w, n]].map(|c| {
                        project(c[0], c[1], center, radius, self.globe_yaw, self.globe_pitch)
                    });
                    if corners.iter().all(|p| p.depth >= 0.0) {
                        let c = if i + 1 == self.selected_pics {
                            palette.accent
                        } else {
                            palette.fill
                        };
                        let mut fill = c;
                        fill[3] = 0.12;
                        draw.add_quad_filled(
                            corners[0].point,
                            corners[1].point,
                            corners[2].point,
                            corners[3].point,
                            fill,
                        );
                        draw.add_quad(
                            corners[0].point,
                            corners[1].point,
                            corners[2].point,
                            corners[3].point,
                            c,
                            1.4,
                            dear_app::imgui::DrawFlags::NONE,
                        );
                    }
                }
                draw.add_circle(p.point, 3.5, palette.accent)
                    .filled(true)
                    .build();
                let d = (p.point[0] - mouse[0]).powi(2) + (p.point[1] - mouse[1]).powi(2);
                if ui.is_item_hovered() && d < 100.0 {
                    hovered_pics = true;
                    ui.tooltip_text(format!(
                        "{} ({})\n{:.3}°, {:.3}°",
                        site.name, site.country, site.center[1], site.center[0]
                    ));
                }
            }
        }
        drop(clip);
        drop(draw);
        if let Some(Ok(data)) = map_data {
            draw_heatmap_legend(
                ui,
                plot_ctx,
                [
                    cursor[0] + area[0] + legend_gap,
                    cursor[1] + PLOT_NAV_HEIGHT,
                ],
                [legend_width, (area[1] - PLOT_NAV_HEIGHT).max(1.0)],
                range,
                false,
                self.settings.heatmap_colormap_preset,
                palette,
                self.settings.display_decimals,
            );
            if let Some((p, _)) = nearest.filter(|_| !hovered_pics) {
                ui.tooltip_text(format!(
                    "Lat {:.3}°, Lon {:.3}°\n{}: {}",
                    p.latitude,
                    p.longitude,
                    if data.aggregated {
                        "Mean value"
                    } else {
                        "Value"
                    },
                    p.special.map_or_else(
                        || format_value(p.value, self.settings.display_decimals),
                        |v| v.label().into()
                    )
                ));
            }
        } else if let Some(Err(error)) = map_data {
            ui.text(format!("Globe data unavailable: {error}"));
        } else if source.is_none() {
            ui.text("Select a geolocated dataset to display it on the globe.");
        }
        let nav_width = if legend_width > 0.0 {
            legend_width
        } else {
            110.0_f32.min(available[0])
        };
        let controls_cursor = ui.cursor_screen_pos();
        let (nav, visibility_changed) = draw_visibility_navigation(
            ui,
            "globe",
            [cursor[0] + available[0] - nav_width, cursor[1]],
            nav_width,
            self.globe_zoom,
            &mut self.value_visibility,
            palette,
        );
        if visibility_changed {
            self.profile_cache = None;
        }
        match nav {
            PlotNavAction::Reset => {
                self.globe_yaw = 0.0;
                self.globe_pitch = 0.0;
                self.globe_zoom = 1.0;
            }
            PlotNavAction::ZoomOut | PlotNavAction::ZoomIn => {
                let step = if nav == PlotNavAction::ZoomIn { 1 } else { -1 };
                self.globe_zoom = (self.globe_zoom * self.settings.globe_zoom_factor.powi(step))
                    .clamp(self.settings.globe_zoom_min, self.settings.globe_zoom_max);
            }
            PlotNavAction::None => {}
        }
        ui.set_cursor_screen_pos(controls_cursor);
        let changes = draw_geo_controls(
            ui,
            "globe",
            &mut self.globe_controls,
            source.as_ref().map(|source| source.shape.as_slice()),
            self.axis_names.get(3).map_or("axis 3", String::as_str),
            has_bounds,
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
                .and_then(|i| self.settings.pics_sites.get(i))
            {
                self.globe_yaw = -site.center[0].to_radians();
                self.globe_pitch = site.center[1].to_radians().clamp(-1.5, 1.5);
                self.globe_zoom = self
                    .settings
                    .map_site_zoom
                    .min(self.settings.globe_zoom_max);
            } else {
                self.globe_yaw = 0.0;
                self.globe_pitch = 0.0;
                self.globe_zoom = 1.0;
            }
        }
        if changes.refresh_data {
            self.map_job = None;
        }
        self.globe_footer_height = (ui.cursor_pos_y() - content_start_y - area[1]).max(1.0);
    }
}
