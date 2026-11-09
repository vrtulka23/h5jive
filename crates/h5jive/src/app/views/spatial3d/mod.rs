use super::super::*;
use dear_app::imgui::{InvisibleButtonMouseButtons, InvisibleButtonOptions, MouseButton};

mod controls;
mod rendering;
#[cfg(test)]
mod tests;

use rendering::*;

impl App {
    fn poll_volume(&mut self, key: &VolumeKey, shape: &[usize]) {
        if self
            .volume_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == key)
        {
            return;
        }
        self.volume_cache = None;
        if self
            .volume_job
            .as_ref()
            .is_some_and(|(working, _)| working != key)
        {
            self.volume_job = None;
        }
        if let Some((_, receiver)) = &self.volume_job {
            match receiver.try_recv() {
                Ok(data) => {
                    self.volume_cache = Some((key.clone(), data));
                    self.volume_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.volume_cache = Some((key.clone(), Err("3D worker stopped".into())));
                    self.volume_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.volume_cache.is_none() && self.volume_job.is_none() {
            let path = self.file_paths[key.file].clone();
            let dataset_path = key.path.clone();
            let shape = shape.to_vec();
            let slice = key.slice.clone();
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let result = open_dataset_reader(&path).and_then(|file| {
                    load_exact_volume(&file, &dataset_path, &shape, &slice).map(Arc::new)
                });
                let _ = sender.send(result);
            });
            self.volume_job = Some((key.clone(), receiver));
        }
    }

    fn draw_volume_canvas(
        &mut self,
        ui: &Ui,
        _plot_ui: &PlotUi<'_>,
        plot_ctx: &PlotContext,
        selection: &Selection,
        shape: &[usize],
    ) {
        let Some(slice) = self.applied_volume.clone() else {
            return;
        };
        let key = VolumeKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
        };
        self.poll_volume(&key, shape);
        let available = ui.content_region_avail();
        if available[0] < 180.0 || available[1] < 150.0 {
            ui.text_wrapped("Enlarge the preview panel to show the volume.");
            return;
        }
        let legend_width = 126.0_f32.min(available[0] * 0.26);
        let size = [available[0] - legend_width - PANEL_GAP, available[1] - 2.0];
        let origin = ui.cursor_screen_pos();
        ui.invisible_button_options(
            "##volume_orbit",
            size,
            InvisibleButtonOptions::new().mouse_buttons(
                InvisibleButtonMouseButtons::LEFT
                    | InvisibleButtonMouseButtons::RIGHT
                    | InvisibleButtonMouseButtons::MIDDLE,
            ),
        );
        let hovered = ui.is_item_hovered();
        let base_center = [origin[0] + size[0] * 0.5, origin[1] + size[1] * 0.5];
        let panning = ui.is_item_active()
            && (ui.is_mouse_dragging(MouseButton::Right)
                || ui.is_mouse_dragging(MouseButton::Middle)
                || ui.io().key_shift() && ui.is_mouse_dragging(MouseButton::Left));
        if panning {
            let delta = ui.io().mouse_delta();
            self.volume_pan[0] += delta[0];
            self.volume_pan[1] += delta[1];
            ui.set_mouse_cursor(Some(MouseCursor::ResizeAll));
        }
        let screen_center = [
            base_center[0] + self.volume_pan[0],
            base_center[1] + self.volume_pan[1],
        ];
        if !panning && ui.is_item_active() && ui.is_mouse_dragging(MouseButton::Left) {
            let current = ui.mouse_pos();
            let delta = ui.io().mouse_delta();
            let previous = [current[0] - delta[0], current[1] - delta[1]];
            self.volume_rotation = self.volume_rotation.drag(
                previous,
                current,
                screen_center,
                size[0].min(size[1]) * 0.45,
            );
            self.volume_projection = None;
        }
        if hovered {
            self.volume_zoom = (self.volume_zoom * (1.0 + ui.io().mouse_wheel() * 0.1))
                .clamp(0.35, self.settings.volume_max_zoom);
        }
        let draw = ui.get_window_draw_list();
        draw.add_rect(
            origin,
            [origin[0] + size[0], origin[1] + size[1]],
            self.settings.palette.border,
        )
        .build();
        let clip = draw.push_clip_rect(origin, [origin[0] + size[0], origin[1] + size[1]], true);
        let base_scale = size[0].min(size[1]) * 0.53 * self.volume_zoom;
        let ratios = volume_axis_ratios(&slice, shape);
        let scale = ratios.map(|ratio| base_scale * ratio);
        let bounds = volume_visible_bounds(&slice, shape);
        let center = volume_projection_center(
            screen_center,
            volume_visible_center(&slice, shape),
            scale,
            self.volume_rotation,
        );
        draw_volume_box(
            &draw,
            center,
            scale,
            self.volume_rotation,
            bounds,
            &self.settings.palette,
            self.settings.volume_back_brightness,
        );
        let mut displayed_range = None;
        let cached = self
            .volume_cache
            .as_ref()
            .map(|(_, result)| result.as_ref().map(Arc::clone).map_err(Clone::clone));
        match cached {
            Some(Ok(data)) => {
                let range = if self.settings.heatmap_auto_range {
                    data.color_range
                } else {
                    Some((
                        self.settings.heatmap_value_min,
                        self.settings.heatmap_value_max,
                    ))
                };
                displayed_range = range;
                let palette = &self.settings.palette;
                let raster_key = VolumeRasterKey {
                    volume: key.clone(),
                    size: [
                        size[0].ceil().max(1.0) as u32,
                        size[1].ceil().max(1.0) as u32,
                    ],
                    center: [center[0] - origin[0], center[1] - origin[1]],
                    scale,
                    rotation: self.volume_rotation.0,
                    opacity: self.volume_opacity,
                    range,
                    logarithmic: self.volume_log_scale,
                    visibility: self.volume_visibility,
                    preset: self.settings.heatmap_colormap_preset,
                    color_stops: [
                        palette.heatmap_low,
                        palette.heatmap_mid,
                        palette.heatmap_high,
                    ],
                    special_colors: [
                        palette.fill,
                        palette.nan,
                        palette.positive_inf,
                        palette.negative_inf,
                    ],
                };
                let raster_is_current = self
                    .volume_raster
                    .as_ref()
                    .is_some_and(|(cached, _)| cached == &raster_key);
                let texture = self.volume_raster_id(&raster_key);
                if !raster_is_current {
                    let colors = std::array::from_fn(|index| {
                        continuous_color(
                            plot_ctx,
                            index as f32 / 255.0,
                            self.settings.heatmap_colormap_preset,
                            palette,
                        )
                    });
                    self.request_volume_raster(VolumeRasterRequest {
                        key: raster_key,
                        data: Arc::clone(&data),
                        colors,
                    });
                }
                if let Some(texture) = texture {
                    draw.add_image(
                        texture,
                        origin,
                        [origin[0] + size[0], origin[1] + size[1]],
                        [0.0, 0.0],
                        [1.0, 1.0],
                        [1.0; 4],
                    );
                } else {
                    draw.add_text(
                        [origin[0] + 8.0, origin[1] + 28.0],
                        self.settings.palette.text,
                        "Rendering exact voxel data...",
                    );
                }
                draw.add_text(
                    [origin[0] + 8.0, origin[1] + 8.0],
                    self.settings.palette.text,
                    format!(
                        "{} voxels | left drag: rotate | right drag: move | wheel: zoom",
                        data.values.len()
                    ),
                );
            }
            Some(Err(error)) => draw.add_text(
                [origin[0] + 8.0, origin[1] + 8.0],
                self.settings.palette.text,
                error,
            ),
            None => draw.add_text(
                [origin[0] + 8.0, origin[1] + 8.0],
                self.settings.palette.text,
                "Loading 3D preview...",
            ),
        }
        let axis_marks = self.volume_axis_marks(selection, &slice, shape, bounds, center, scale);
        for mark in &axis_marks {
            draw_axis_mark(
                ui,
                &draw,
                mark,
                center,
                scale,
                self.volume_rotation,
                self.settings.palette.text,
            );
        }
        drop(clip);
        draw_volume_legend(
            &draw,
            [origin[0] + size[0] + PANEL_GAP, origin[1] + PLOT_NAV_HEIGHT],
            [legend_width, (size[1] - PLOT_NAV_HEIGHT).max(1.0)],
            displayed_range,
            self.volume_log_scale,
            &self.settings.palette,
            self.settings.heatmap_colormap_preset,
            plot_ctx,
            self.settings.display_decimals,
        );
        drop(draw);
        let (nav, visibility_changed) = draw_visibility_navigation(
            ui,
            "volume",
            [origin[0] + available[0] - legend_width, origin[1]],
            legend_width,
            self.volume_zoom,
            &mut self.volume_visibility,
            &self.settings.palette,
        );
        if visibility_changed {
            self.profile_cache = None;
        }
        match nav {
            PlotNavAction::Reset => {
                self.volume_rotation = VolumeRotation::from_angles(0.65, 0.45);
                self.volume_axis_corner = [0.5; 3];
                self.volume_projection = Some(VolumeProjection::Isometric);
                self.volume_zoom = 1.0;
                self.volume_pan = [0.0, 0.0];
            }
            PlotNavAction::ZoomOut => {
                self.volume_zoom =
                    (self.volume_zoom / 1.1).clamp(0.35, self.settings.volume_max_zoom);
            }
            PlotNavAction::ZoomIn => {
                self.volume_zoom =
                    (self.volume_zoom * 1.1).clamp(0.35, self.settings.volume_max_zoom);
            }
            PlotNavAction::None => {}
        }
    }

    fn volume_axis_marks(
        &mut self,
        selection: &Selection,
        slice: &VolumeSlice,
        shape: &[usize],
        bounds: [[f32; 2]; 3],
        center: [f32; 2],
        scale: [f32; 3],
    ) -> Vec<AxisMark> {
        let mut marks = Vec::new();
        // Put the axes on the volume edges and offset only their ticks and labels.
        let origin = front_axis_corner(self.volume_rotation, self.volume_axis_corner, bounds);
        for slot in 0..slice.visible_count() {
            let axis = slice.axes[slot].unwrap();
            let outward = (slot + 1) % 3;
            let mut end = origin;
            end[slot] = if origin[slot] == bounds[slot][0] {
                bounds[slot][1]
            } else {
                bounds[slot][0]
            };
            let from_screen = project_rotated(origin, center, scale, self.volume_rotation).0;
            let to_screen = project_rotated(end, center, scale, self.volume_rotation).0;
            let pixels = (to_screen[0] - from_screen[0]).hypot(to_screen[1] - from_screen[1]);
            // Short segments let each part of an axis take its own place in depth order.
            for segment in 0..16 {
                let mut from = origin;
                let mut to = origin;
                from[slot] = origin[slot] + (end[slot] - origin[slot]) * segment as f32 / 16.0;
                to[slot] = origin[slot] + (end[slot] - origin[slot]) * (segment + 1) as f32 / 16.0;
                marks.push(AxisMark::Line {
                    from,
                    to,
                    thickness: 1.5,
                });
            }
            let mut far = end;
            let offset = |axis: usize, pixels: f32| pixels / scale[axis].max(1.0);
            far[slot] += (origin[slot] - end[slot]).signum() * offset(slot, 9.0);
            let outward_direction = if origin[outward] == bounds[outward][1] {
                1.0
            } else {
                -1.0
            };
            far[outward] += outward_direction * offset(outward, 18.0);
            marks.push(AxisMark::AxisName {
                point: far,
                label: format!(
                    "{}: {}",
                    ["X", "Y", "Z"][slot],
                    axis_name(&self.axis_names, axis)
                ),
            });
            let tick_budget = if pixels < 90.0 {
                1
            } else if pixels < 180.0 {
                2
            } else if pixels < 300.0 {
                3
            } else {
                4
            };
            let [first, last] = slice.ranges[axis];
            let ticks = if tick_budget == 1 {
                vec![first + (last - first) / 2]
            } else {
                axis_ticks(last - first + 1, tick_budget)
                    .into_iter()
                    .map(|index| first + index)
                    .collect()
            };
            for index in ticks {
                let mut point = origin;
                point[slot] = -axis_center(index, shape[axis]);
                let mut tip = point;
                tip[outward] += outward_direction * offset(outward, 7.0);
                marks.push(AxisMark::Line {
                    from: point,
                    to: tip,
                    thickness: 1.3,
                });
                tip[outward] += outward_direction * offset(outward, 11.0);
                marks.push(AxisMark::Text {
                    point: tip,
                    label: self.coordinate_label(selection, axis, index),
                });
            }
        }
        marks
    }
}

enum AxisMark {
    Line {
        from: [f32; 3],
        to: [f32; 3],
        thickness: f32,
    },
    Text {
        point: [f32; 3],
        label: String,
    },
    AxisName {
        point: [f32; 3],
        label: String,
    },
}
