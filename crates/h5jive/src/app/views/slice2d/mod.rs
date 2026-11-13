use super::super::*;

mod layout;
mod legend;

pub(crate) use layout::*;
pub(crate) use legend::*;

fn draw_native_heatmap_tile(
    ui: &Ui,
    plot_ui: &PlotUi<'_>,
    image: &HeatmapImage,
    visible: [usize; 4],
    limits: [f64; 4],
    range: (f64, f64),
    visibility: ValueVisibility,
    palette: &Palette,
) {
    let [r0, r1, c0, c1] = image.bounds;
    let (min, max) = range;
    let scale_max = if max == min {
        max + f64::EPSILON.max(max.abs() * 1e-12)
    } else {
        max
    };
    let label = format!("native tile {r0} {r1} {c0} {c1}");
    plot_heatmap_grid(
        plot_ui,
        &label,
        &image.plotted,
        [r1 - r0, c1 - c0],
        (min, scale_max),
        [c0 as f64, r0 as f64, c1 as f64, r1 as f64],
    );
    let draw = ui.get_window_draw_list();
    let upper_left = plot_ui.plot_to_pixels_axes(
        ImPlotPoint {
            x: limits[0],
            y: limits[3],
        },
        XAxis::X1,
        YAxis::Y1,
    );
    let lower_right = plot_ui.plot_to_pixels_axes(
        ImPlotPoint {
            x: limits[1],
            y: limits[2],
        },
        XAxis::X1,
        YAxis::Y1,
    );
    let _clip = draw.push_clip_rect(upper_left, lower_right, true);
    for row in r0.max(visible[0])..r1.min(visible[1]) {
        for column in c0.max(visible[2])..c1.min(visible[3]) {
            let value = image.values[(row - r0) * (c1 - c0) + column - c0];
            let special = SpecialValue::from_value(value, image.fill_value);
            if visibility.allows_class(value, special) && special.is_none() {
                continue;
            }
            let first = plot_ui.plot_to_pixels_axes(
                ImPlotPoint {
                    x: column as f64,
                    y: (row + 1) as f64,
                },
                XAxis::X1,
                YAxis::Y1,
            );
            let last = plot_ui.plot_to_pixels_axes(
                ImPlotPoint {
                    x: (column + 1) as f64,
                    y: row as f64,
                },
                XAxis::X1,
                YAxis::Y1,
            );
            let color = if visibility.allows_class(value, special) {
                special.unwrap().color(palette)
            } else {
                palette.background
            };
            draw.add_rect(first, last, color).filled(true).build();
        }
    }
}

impl App {
    fn draw_cached_heatmap_tiles(
        &self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        selection: &Selection,
        slice: &TableSlice,
        rows: usize,
        visible: [usize; 4],
        limits: [f64; 4],
        range: (f64, f64),
    ) {
        for (key, image) in &self.heatmap_tiles {
            let [r0, r1, c0, c1] = key.bounds;
            if key.file == selection.file
                && key.path == selection.path
                && &key.slice == slice
                && r0 < visible[1]
                && visible[0] < r1
                && c0 < visible[3]
                && visible[2] < c1
            {
                draw_native_heatmap_tile(
                    ui,
                    plot_ui,
                    image,
                    visible,
                    limits,
                    range,
                    self.value_visibility,
                    &self.settings.palette,
                );
            }
        }
        if plot_ui.is_plot_hovered() {
            let mouse = plot_ui.plot_mouse_position_axes(XAxis::X1, YAxis::Y1);
            if mouse.x >= 0.0 && mouse.y >= 0.0 && mouse.y < rows as f64 {
                let column = mouse.x.floor() as usize;
                let row = mouse.y.floor() as usize;
                if let Some((key, image)) = self.heatmap_tiles.iter().rev().find(|(key, _)| {
                    key.file == selection.file
                        && key.path == selection.path
                        && &key.slice == slice
                        && row >= key.bounds[0]
                        && row < key.bounds[1]
                        && column >= key.bounds[2]
                        && column < key.bounds[3]
                }) {
                    let [r0, _, c0, c1] = key.bounds;
                    let value = image.values[(row - r0) * (c1 - c0) + column - c0];
                    let special = SpecialValue::from_value(value, image.fill_value);
                    if self.value_visibility.allows_class(value, special) {
                        ui.set_tooltip(format!(
                            "Row {row}, column {column}: {}",
                            special.map_or_else(
                                || format_value(value, self.settings.display_decimals),
                                |special| special.label().to_owned()
                            )
                        ));
                    }
                }
            }
        }
    }

    pub(crate) fn draw_heatmap(
        &mut self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        plot_ctx: &PlotContext,
        selection: &Selection,
        shape: &[usize],
    ) {
        let Some(slice) = self.applied_slice.clone() else {
            return;
        };
        let Some(row_axis) = slice.row_axis else {
            ui.text("Select two axes for the heatmap.");
            return;
        };
        if shape.iter().any(|&size| size == 0) {
            ui.text("Empty dataset");
            return;
        }
        let rows = slice.rows(shape);
        let columns = slice.columns(shape);
        let max_cells = self.settings.heatmap_max_cells.min(1_000_000);
        let large_plane = rows.saturating_mul(columns) > max_cells;
        if !large_plane {
            self.poll_heatmap_range(selection, shape, &slice);
        } else {
            self.heatmap_range_job = None;
        }
        let mut display_range = if self.settings.heatmap_auto_range {
            if large_plane {
                self.heatmap_overview_cache
                    .as_ref()
                    .and_then(|(key, result)| {
                        if key.file != selection.file
                            || key.path != selection.path
                            || key.slice != slice
                        {
                            return None;
                        }
                        result.as_ref().ok().and_then(|image| image.range)
                    })
            } else {
                self.heatmap_range_cache
                    .as_ref()
                    .and_then(|(_, result)| result.as_ref().ok().copied().flatten())
            }
        } else {
            Some((
                self.settings.heatmap_value_min,
                self.settings.heatmap_value_max,
            ))
        };
        let range_provisional = self.settings.heatmap_auto_range && display_range.is_none();
        let available = ui.content_region_avail();
        let legend_width = self
            .settings
            .heatmap_legend_width
            .min(available[0] * self.settings.heatmap_legend_fraction);
        let plot_width = (available[0] - legend_width - PANEL_GAP).max(1.0);
        let panel_origin = ui.cursor_screen_pos();
        if plot_width < 180.0 || available[1] < 150.0 {
            ui.text_wrapped("Enlarge the plot panel to show the heatmap.");
            return;
        }
        let _colormap = if self.settings.heatmap_colormap_preset == ColormapPreset::Viridis {
            plot_ui.push_colormap(Colormap::Viridis)
        } else {
            let colors = [
                self.settings.palette.heatmap_low,
                self.settings.palette.heatmap_mid,
                self.settings.palette.heatmap_high,
            ];
            if self
                .heatmap_colormap
                .as_ref()
                .is_none_or(|(old, _)| old != &colors)
            {
                let name = format!(
                    "h5jive_map_{}",
                    self.heatmap_colormap.as_ref().map_or(0, |(_, name)| name
                        .rsplit('_')
                        .next()
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(0)
                        + 1)
                );
                plot_ctx.add_colormap(&name, &colors, false);
                self.heatmap_colormap = Some((colors, name));
            }
            plot_ui.push_colormap_name(&self.heatmap_colormap.as_ref().unwrap().1)
        };
        if rows.saturating_mul(columns) > max_cells {
            let (_, grid) = heatmap_layout(
                rows,
                columns,
                [plot_width - 75.0, available[1] - 55.0],
                self.settings.heatmap_cell_size,
                max_cells,
            );
            self.poll_heatmap_overview(selection, shape, &slice, [grid[1], grid[0]]);
        }
        let x_name = axis_name(&self.axis_names, slice.column_axis);
        let y_name = axis_name(&self.axis_names, row_axis);
        let tick_key = HeatmapRangeKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
        };
        let tick_bounds = self
            .heatmap_tick_view
            .as_ref()
            .filter(|(key, _)| key == &tick_key)
            .map(|(_, limits)| heatmap_visible_bounds(rows, columns, *limits))
            .unwrap_or([0, rows, 0, columns]);
        let x_indices = heatmap_axis_ticks(tick_bounds[2], tick_bounds[3], 6);
        let y_indices = heatmap_axis_ticks(tick_bounds[0], tick_bounds[1], 6);
        let x_positions = x_indices
            .iter()
            .map(|&index| index as f64 + 0.5)
            .collect::<Vec<_>>();
        let y_positions = y_indices
            .iter()
            .map(|&index| index as f64 + 0.5)
            .collect::<Vec<_>>();
        let x_labels = x_indices
            .iter()
            .map(|&index| self.coordinate_label(selection, slice.column_axis, index))
            .collect::<Vec<_>>();
        let y_labels = y_indices
            .iter()
            .map(|&index| self.coordinate_label(selection, row_axis, index))
            .collect::<Vec<_>>();
        let x_label_refs = x_labels.iter().map(String::as_str).collect::<Vec<_>>();
        let y_label_refs = y_labels.iter().map(String::as_str).collect::<Vec<_>>();
        let title = format!("##heatmap-{}-{}-{slice:?}", selection.file, selection.path);
        let Some(_plot) = plot_ui.begin_plot_with_size(&title, [plot_width, available[1] - 2.0])
        else {
            return;
        };
        plot_ui.setup_axes(
            Some(&x_name),
            Some(&y_name),
            AxisFlags::NONE,
            AxisFlags::NONE,
        );
        plot_ui.setup_x_axis_ticks_positions(XAxis::X1, &x_positions, Some(&x_label_refs), false);
        plot_ui.setup_y_axis_ticks_positions(YAxis::Y1, &y_positions, Some(&y_label_refs), false);
        let full_limits = [0.0, columns as f64, 0.0, rows as f64];
        if let Some((_, nav_limits)) = self
            .heatmap_nav_override
            .take()
            .filter(|(key, _)| key == &tick_key)
        {
            plot_ui.setup_axes_limits(
                nav_limits[0],
                nav_limits[1],
                nav_limits[2],
                nav_limits[3],
                PlotCond::Always,
            );
        } else {
            plot_ui.setup_axes_limits(0.0, columns as f64, 0.0, rows as f64, PlotCond::Once);
        }
        plot_ui.setup_finish();
        let limits = plot_ui.plot_limits_axes(XAxis::X1, YAxis::Y1);
        let current_limits = [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max];
        if self
            .heatmap_tick_view
            .as_ref()
            .is_some_and(|(previous_key, previous)| {
                previous_key == &tick_key
                    && previous
                        .iter()
                        .zip(current_limits)
                        .any(|(&old, new)| (old - new).abs() > 1e-6)
            })
        {
            self.note_heatmap_navigation();
        }
        self.heatmap_tick_view = Some((tick_key.clone(), current_limits));
        let visible = heatmap_visible_bounds(
            rows,
            columns,
            [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max],
        );
        let visible_count = (visible[1] - visible[0]).saturating_mul(visible[3] - visible[2]);
        if display_range.is_none() && range_provisional {
            display_range = self.heatmap_tiles.iter().rev().find_map(|(cached, image)| {
                let [r0, r1, c0, c1] = cached.bounds;
                (cached.file == selection.file
                    && cached.path == selection.path
                    && cached.slice == slice
                    && r0 < visible[1]
                    && visible[0] < r1
                    && c0 < visible[3]
                    && visible[2] < c1)
                    .then_some(image.range)
                    .flatten()
            });
        }
        if visible_count > 0 && visible_count <= max_cells {
            let bounds = heatmap_tile_bounds(rows, columns, visible, max_cells);
            let key = HeatmapKey {
                file: selection.file,
                path: selection.path.clone(),
                slice: slice.clone(),
                bounds,
            };
            self.poll_heatmap_tile(
                key.clone(),
                shape,
                display_range.map_or(0.0, |(min, _)| min),
                Self::refinement_settled(self.heatmap_navigation_at),
            );
            let tile = self
                .heatmap_cache
                .as_ref()
                .filter(|(cached, _)| cached == &key)
                .map(|(_, result)| result);
            if let Some(Ok(image)) = tile {
                let [r0, r1, c0, c1] = image.bounds;
                if display_range.is_none() && range_provisional {
                    display_range = image.range;
                }
                draw_native_heatmap_tile(
                    ui,
                    plot_ui,
                    image,
                    visible,
                    [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max],
                    display_range.unwrap_or((0.0, 1.0)),
                    self.value_visibility,
                    &self.settings.palette,
                );
                if plot_ui.is_plot_hovered() {
                    let mouse = plot_ui.plot_mouse_position_axes(XAxis::X1, YAxis::Y1);
                    if mouse.x >= 0.0
                        && mouse.x < columns as f64
                        && mouse.y >= 0.0
                        && mouse.y < rows as f64
                    {
                        let column = mouse.x.floor() as usize;
                        let row = mouse.y.floor() as usize;
                        if row >= r0 && row < r1 && column >= c0 && column < c1 {
                            let value = image.values[(row - r0) * (c1 - c0) + column - c0];
                            let special = SpecialValue::from_value(value, image.fill_value);
                            if self.value_visibility.allows_class(value, special) {
                                let row_label = self.coordinate_label(selection, row_axis, row);
                                let column_label =
                                    self.coordinate_label(selection, slice.column_axis, column);
                                ui.set_tooltip(format!(
                                    "Row {row_label}, column {column_label}: {}",
                                    special.map_or_else(
                                        || format_value(value, self.settings.display_decimals),
                                        |special| special.label().to_owned()
                                    )
                                ));
                            }
                        }
                    }
                }
            } else {
                if let Some(Ok(image)) = self
                    .heatmap_overview_cache
                    .as_ref()
                    .filter(|(cached, _)| {
                        cached.file == selection.file
                            && cached.path == selection.path
                            && cached.slice == slice
                    })
                    .map(|(_, result)| result)
                {
                    let (min, max) = display_range.unwrap_or((0.0, 1.0));
                    let scale_max = if max == min {
                        max + f64::EPSILON.max(max.abs() * 1e-12)
                    } else {
                        max
                    };
                    plot_heatmap_grid(
                        plot_ui,
                        "overview while loading",
                        &image.plotted,
                        image.grid,
                        (min, scale_max),
                        [0.0, 0.0, columns as f64, rows as f64],
                    );
                }
                self.draw_cached_heatmap_tiles(
                    ui,
                    plot_ui,
                    selection,
                    &slice,
                    rows,
                    visible,
                    [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max],
                    display_range.unwrap_or((0.0, 1.0)),
                );
                let pos = plot_ui.plot_to_pixels_axes(
                    ImPlotPoint {
                        x: limits.X.Min,
                        y: limits.Y.Max,
                    },
                    XAxis::X1,
                    YAxis::Y1,
                );
                ui.get_window_draw_list().add_text(
                    [pos[0] + 10.0, pos[1] + 10.0],
                    self.settings.palette.text,
                    match tile {
                        Some(Err(error)) => error.as_str(),
                        _ => "Loading 2D tile...",
                    },
                );
            }
        } else if visible_count > max_cells {
            let pos = plot_ui.plot_to_pixels_axes(
                ImPlotPoint {
                    x: limits.X.Min,
                    y: limits.Y.Max,
                },
                XAxis::X1,
                YAxis::Y1,
            );
            match self
                .heatmap_overview_cache
                .as_ref()
                .map(|(_, result)| result)
            {
                Some(Ok(image)) => {
                    if display_range.is_none() && range_provisional {
                        display_range = image.range;
                    }
                    let (min, max) = display_range.unwrap_or((0.0, 1.0));
                    plot_heatmap_grid(
                        plot_ui,
                        "overview",
                        &image.plotted,
                        image.grid,
                        (min, max),
                        [0.0, 0.0, columns as f64, rows as f64],
                    );
                    let draw = ui.get_window_draw_list();
                    let lower_right = plot_ui.plot_to_pixels_axes(
                        ImPlotPoint {
                            x: limits.X.Max,
                            y: limits.Y.Min,
                        },
                        XAxis::X1,
                        YAxis::Y1,
                    );
                    let _clip = draw.push_clip_rect(pos, lower_right, true);
                    for row_bin in 0..image.grid[0] {
                        for column_bin in 0..image.grid[1] {
                            let value = image.values[row_bin * image.grid[1] + column_bin];
                            let special = SpecialValue::from_value(value, image.fill_value);
                            if self.value_visibility.allows_class(value, special)
                                && special.is_none()
                            {
                                continue;
                            }
                            let x0 = column_bin as f64 * columns as f64 / image.grid[1] as f64;
                            let x1 =
                                (column_bin + 1) as f64 * columns as f64 / image.grid[1] as f64;
                            let y0 = (row_bin + 1) as f64 * rows as f64 / image.grid[0] as f64;
                            let y1 = row_bin as f64 * rows as f64 / image.grid[0] as f64;
                            let first = plot_ui.plot_to_pixels_axes(
                                ImPlotPoint { x: x0, y: y0 },
                                XAxis::X1,
                                YAxis::Y1,
                            );
                            let last = plot_ui.plot_to_pixels_axes(
                                ImPlotPoint { x: x1, y: y1 },
                                XAxis::X1,
                                YAxis::Y1,
                            );
                            let color = if self.value_visibility.allows_class(value, special) {
                                special.unwrap().color(&self.settings.palette)
                            } else {
                                self.settings.palette.background
                            };
                            draw.add_rect(first, last, color).filled(true).build();
                        }
                    }
                    drop(_clip);
                    drop(draw);
                    if plot_ui.is_plot_hovered() {
                        let mouse = plot_ui.plot_mouse_position_axes(XAxis::X1, YAxis::Y1);
                        if mouse.x >= 0.0
                            && mouse.x < columns as f64
                            && mouse.y >= 0.0
                            && mouse.y < rows as f64
                        {
                            let column_bin = ((mouse.x / columns as f64 * image.grid[1] as f64)
                                .floor() as usize)
                                .min(image.grid[1] - 1);
                            let row_bin = ((mouse.y / rows as f64 * image.grid[0] as f64).floor()
                                as usize)
                                .min(image.grid[0] - 1);
                            let row = overview_sample_index(row_bin, rows, image.grid[0]);
                            let column = overview_sample_index(column_bin, columns, image.grid[1]);
                            let value = image.values[row_bin * image.grid[1] + column_bin];
                            let special = SpecialValue::from_value(value, image.fill_value);
                            if self.value_visibility.allows_class(value, special) {
                                let row_label = self.coordinate_label(selection, row_axis, row);
                                let column_label =
                                    self.coordinate_label(selection, slice.column_axis, column);
                                ui.set_tooltip(format!(
                                    "Sampled row {row_label}, column {column_label}: {}",
                                    special.map_or_else(
                                        || format_value(value, self.settings.display_decimals),
                                        |special| special.label().to_owned()
                                    )
                                ));
                            }
                        }
                    }
                }
                Some(Err(error)) => {
                    ui.get_window_draw_list().add_text(
                        [pos[0] + 10.0, pos[1] + 10.0],
                        self.settings.palette.text,
                        error,
                    );
                }
                None => {
                    ui.get_window_draw_list().add_text(
                        [pos[0] + 10.0, pos[1] + 10.0],
                        self.settings.palette.text,
                        "Loading full image...",
                    );
                }
            }
            self.draw_cached_heatmap_tiles(
                ui,
                plot_ui,
                selection,
                &slice,
                rows,
                visible,
                [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max],
                display_range.unwrap_or((0.0, 1.0)),
            );
        }
        drop(_plot);
        ui.set_cursor_screen_pos([
            panel_origin[0] + available[0] - legend_width,
            panel_origin[1],
        ]);
        let origin = ui.cursor_screen_pos();
        draw_heatmap_legend(
            ui,
            plot_ctx,
            [origin[0], origin[1] + PLOT_NAV_HEIGHT],
            [legend_width, (available[1] - PLOT_NAV_HEIGHT).max(1.0)],
            display_range,
            range_provisional,
            self.settings.heatmap_colormap_preset,
            &self.settings.palette,
            self.settings.display_decimals,
        );
        ui.dummy([legend_width, available[1] - 2.0]);
        let x_span = (limits.X.Max - limits.X.Min).max(1.0);
        let y_span = (limits.Y.Max - limits.Y.Min).max(1.0);
        let zoom = ((columns as f64 / x_span) * (rows as f64 / y_span)).sqrt() as f32;
        let (nav, visibility_changed) = draw_visibility_navigation(
            ui,
            "heatmap",
            origin,
            legend_width,
            zoom,
            &mut self.value_visibility,
            &self.settings.palette,
        );
        if visibility_changed {
            self.profile_cache = None;
        }
        let current = [limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max];
        let next = match nav {
            PlotNavAction::Reset => Some(full_limits),
            PlotNavAction::ZoomOut => Some(heatmap_zoom_limits(current, full_limits, 0.8)),
            PlotNavAction::ZoomIn => Some(heatmap_zoom_limits(current, full_limits, 1.25)),
            PlotNavAction::None => None,
        };
        if let Some(next) = next {
            self.heatmap_nav_override = Some((tick_key, next));
        }
    }
}
