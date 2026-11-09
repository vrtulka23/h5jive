use super::super::*;

impl App {
    pub(crate) fn draw_profile_view(&mut self, ui: &Ui) {
        let Some(selection) = self.selection.clone() else {
            ui.text("Select a numeric dataset from the tree.");
            return;
        };
        let Some(entry) = find_entry(&self.files[selection.file].tree, &selection.path) else {
            return;
        };
        let shape = match &entry.kind {
            Kind::Dataset {
                shape,
                numeric: true,
            } if (1..=4).contains(&shape.len()) => shape.clone(),
            _ => {
                ui.text("Select a 1D to 4D numeric dataset for profiles.");
                return;
            }
        };
        let height = (ui.content_region_avail()[1] - self.profile_footer_height)
            .max(self.settings.panel_min_height);
        let origin = ui.cursor_screen_pos();
        self.view_rect = [origin[0], origin[1], ui.content_region_avail()[0], height];
        ui.child_window("profile_plot_panel")
            .size([0.0, height])
            .build(ui, || {
                self.draw_profile_plot(ui, &selection, &shape);
            });
        let footer_start = ui.cursor_pos_y();
        ui.separator();
        let button_width = control_action_width(ui, &self.settings);
        if let Some(_table) = begin_control_columns(ui, "profile_controls", button_width) {
            self.draw_profile_controls(ui, &shape);
            ui.table_next_column();
            let pending = self.draft_profile != self.applied_profile;
            if draw_update_button(ui, &self.settings, pending) {
                if let Some(draft) = self.draft_profile.as_ref() {
                    let valid = draft.y_axis.map_or(Ok(vec![0]), |axis| {
                        parse_profile_indexes(&draft.y_indexes, shape[axis])
                    });
                    if valid.is_ok() {
                        self.applied_profile = Some(draft.clone());
                        self.profile_cache = None;
                    }
                }
            }
        }
        self.profile_footer_height = (ui.cursor_pos_y() - footer_start).max(1.0);
    }
    pub(crate) fn draw_profile_controls(&mut self, ui: &Ui, shape: &[usize]) {
        let pinned = self
            .draft_profile
            .as_ref()
            .map(|draft| draft.pinned.clone());
        let coordinate_labels = pinned
            .as_ref()
            .map(|pinned| self.pinned_coordinate_labels(pinned));
        let Some(draft) = self.draft_profile.as_mut() else {
            return;
        };
        let labels = axis_option_labels(&self.axis_names, shape);
        let side_by_side = draft.y_axis.is_some() && ui.content_region_avail()[0] >= 360.0;
        ui.text("X");
        ui.same_line();
        ui.set_next_item_width(if side_by_side {
            (ui.content_region_avail()[0] * 0.4).max(90.0)
        } else {
            (ui.content_region_avail()[0] * 0.65).max(60.0)
        });
        if ui.combo_simple_string("##profile_x_axis", &mut draft.x_axis, &labels)
            && draft.y_axis == Some(draft.x_axis)
        {
            draft.y_axis = (0..shape.len()).find(|&axis| axis != draft.x_axis);
            if let Some(axis) = draft.y_axis {
                draft.y_indexes = draft.pinned[axis].to_string();
            }
        }
        if let Some(mut y_axis) = draft.y_axis {
            if side_by_side {
                ui.same_line();
            }
            ui.text("Y profiles");
            ui.same_line();
            ui.set_next_item_width(if side_by_side {
                ui.content_region_avail()[0].max(80.0)
            } else {
                (ui.content_region_avail()[0] * 0.65).max(60.0)
            });
            if ui.combo_simple_string("##profile_y_axis", &mut y_axis, &labels) {
                if y_axis == draft.x_axis {
                    draft.x_axis = (0..shape.len()).find(|&axis| axis != y_axis).unwrap();
                }
                draft.y_axis = Some(y_axis);
                draft.y_indexes = draft.pinned[y_axis].to_string();
            }
            ui.set_next_item_width((ui.content_region_avail()[0] * 0.65).max(60.0));
            ui.input_text("Y indexes", &mut draft.y_indexes).build();
            ui.set_item_tooltip("Comma-separated Y indexes, for example: 0, 3, 7");
            if let Err(error) = parse_profile_indexes(&draft.y_indexes, shape[y_axis]) {
                ui.text_wrapped(error);
            }
        }
        for (axis, &length) in shape.iter().enumerate() {
            if axis == draft.x_axis || draft.y_axis == Some(axis) {
                continue;
            }
            let mut index = draft.pinned[axis] as i32;
            let name = axis_name(&self.axis_names, axis);
            ui.set_next_item_width((ui.content_region_avail()[0] * 0.65).max(60.0));
            ui.slider(
                format!(
                    "{name} {}##profile_pinned_{axis}",
                    coordinate_labels
                        .as_ref()
                        .map_or("", |labels| labels[axis].as_str())
                ),
                0,
                length.saturating_sub(1).min(i32::MAX as usize) as i32,
                &mut index,
            );
            draft.pinned[axis] = index as usize;
        }
    }
    pub(crate) fn draw_profile_plot(&mut self, ui: &Ui, selection: &Selection, shape: &[usize]) {
        let Some(slice) = self.applied_profile.clone() else {
            return;
        };
        if self.profile_cache.is_none() {
            self.profile_cache = Some(load_profile(
                &self.files[selection.file],
                &selection.path,
                shape,
                &slice,
                &self.value_visibility,
            ));
        }
        let (x_len, line_indexes) = match self.profile_cache.as_ref().unwrap() {
            Ok(data) => (
                data.x_len,
                data.lines.iter().map(|line| line.index).collect::<Vec<_>>(),
            ),
            Err(error) => {
                ui.text_wrapped(error);
                return;
            }
        };
        let mut x_labels = HashMap::new();
        for tick_count in 1..=6 {
            for index in axis_ticks(x_len, tick_count) {
                x_labels
                    .entry(index)
                    .or_insert_with(|| self.coordinate_label(selection, slice.x_axis, index));
            }
        }
        let line_labels = line_indexes
            .into_iter()
            .map(|index| {
                slice.y_axis.map_or_else(
                    || "Value".into(),
                    |axis| self.coordinate_label(selection, axis, index),
                )
            })
            .collect::<Vec<_>>();
        let data = self.profile_cache.as_ref().unwrap().as_ref().unwrap();
        let (nav, visibility_changed) = draw_profile_plot(
            ui,
            data,
            &slice,
            &self.axis_names,
            &self.settings.palette,
            &x_labels,
            &line_labels,
            self.settings.display_decimals,
            self.profile_zoom,
            &mut self.value_visibility,
        );
        if visibility_changed {
            self.profile_cache = None;
        }
        self.profile_zoom = match nav {
            PlotNavAction::Reset => 1.0,
            PlotNavAction::ZoomOut => (self.profile_zoom / 1.25).max(1.0),
            PlotNavAction::ZoomIn => (self.profile_zoom * 1.25).min(64.0),
            PlotNavAction::None => self.profile_zoom,
        };
    }
}

pub(crate) fn draw_profile_plot(
    ui: &Ui,
    data: &ProfileData,
    slice: &ProfileSlice,
    axis_names: &[String],
    palette: &Palette,
    x_labels: &HashMap<usize, String>,
    line_labels: &[String],
    decimals: usize,
    zoom: f32,
    visibility: &mut ValueVisibility,
) -> (PlotNavAction, bool) {
    let available = ui.content_region_avail();
    if available[0] < 210.0 || available[1] < 160.0 {
        ui.text_wrapped("Enlarge this panel to see the profile plot.");
        return (PlotNavAction::None, false);
    }
    let legend_width = (available[0] * 0.27).clamp(90.0, 150.0);
    let plot_width = (available[0] - legend_width - PANEL_GAP).max(1.0);
    let panel_origin = ui.cursor_screen_pos();
    ui.child_window("profile_canvas")
        .size([plot_width, available[1]])
        .flags(WindowFlags::NO_SCROLLBAR)
        .build(ui, || {
            if data.min.is_finite() && data.max.is_finite() {
                draw_profile_canvas(
                    ui, data, slice, axis_names, palette, x_labels, decimals, zoom,
                );
            } else {
                ui.text("No finite, non-fill values to plot.");
            }
        });
    ui.set_cursor_screen_pos([
        panel_origin[0] + available[0] - legend_width,
        panel_origin[1],
    ]);
    let mut nav = PlotNavAction::None;
    let mut visibility_changed = false;
    ui.child_window("profile_line_legend")
        .size([legend_width, available[1]])
        .build(ui, || {
            let origin = ui.cursor_screen_pos();
            (nav, visibility_changed) = draw_visibility_navigation(
                ui,
                "profile",
                origin,
                ui.content_region_avail()[0],
                zoom,
                visibility,
                palette,
            );
            ui.set_cursor_screen_pos([origin[0], origin[1] + PLOT_NAV_HEIGHT]);
            if !data.min.is_finite() || !data.max.is_finite() {
                return;
            }
            ui.text_wrapped("Profiles");
            ui.text_wrapped(slice.y_axis.map_or_else(
                || axis_name(axis_names, slice.x_axis),
                |axis| axis_name(axis_names, axis),
            ));
            ui.separator();
            for line_number in 0..data.lines.len() {
                ui.dummy([24.0, ui.text_line_height()]);
                let min = ui.item_rect_min();
                let max = ui.item_rect_max();
                let y = (min[1] + max[1]) * 0.5;
                let draw = ui.get_window_draw_list();
                draw.add_line(
                    [min[0], y],
                    [max[0], y],
                    profile_line_color(palette, line_number),
                )
                .thickness(2.0)
                .build();
                drop(draw);
                ui.same_line();
                ui.text_wrapped(&line_labels[line_number]);
            }
        });
    (nav, visibility_changed)
}
pub(crate) fn profile_line_color(palette: &Palette, index: usize) -> [f32; 4] {
    let colors = [
        palette.accent,
        palette.heatmap_high,
        palette.heatmap_low,
        palette.positive_inf,
        palette.negative_inf,
        mix_color(palette.accent, palette.heatmap_high, 0.5),
    ];
    colors[index % colors.len()]
}
pub(crate) fn draw_profile_canvas(
    ui: &Ui,
    data: &ProfileData,
    slice: &ProfileSlice,
    axis_names: &[String],
    palette: &Palette,
    x_labels: &HashMap<usize, String>,
    decimals: usize,
    zoom: f32,
) {
    let available = ui.content_region_avail();
    let size = [available[0] - 2.0, available[1] - 2.0];
    let line_height = ui.text_line_height();
    let tick_width = x_labels
        .iter()
        .map(|(index, label)| {
            ui.calc_text_size(index.to_string())[0]
                .max(ui.calc_text_size(tick_coordinate(*index, label).unwrap_or(""))[0])
        })
        .fold(0.0_f32, f32::max);
    let side_padding = (tick_width * 0.5 + 8.0).clamp(16.0, 100.0);
    let left = 68.0_f32.max(side_padding);
    let right = size[0] - side_padding;
    let top = 28.0;
    let bottom = size[1] - (line_height * 3.0 + 18.0).max(58.0);
    if bottom <= top + 20.0 || right <= left + 20.0 {
        ui.text_wrapped("Enlarge this panel to see the profile plot.");
        return;
    }
    ui.dummy(size);
    let origin = ui.item_rect_min();
    let x0 = origin[0] + left;
    let x1 = origin[0] + right;
    let y0 = origin[1] + top;
    let y1 = origin[1] + bottom;
    let mut minimum = data.min;
    let mut maximum = data.max;
    if minimum == maximum {
        let margin = (minimum.abs() * 0.05).max(1.0);
        minimum -= margin;
        maximum += margin;
    }
    let x_span = data.x_len as f32 / zoom;
    let x_min = (data.x_len as f32 - x_span) * 0.5;
    let x_position = |bin: usize, count: usize| {
        let index = (bin as f32 + 0.5) / count as f32 * data.x_len as f32;
        x0 + (index - x_min) / x_span * (x1 - x0)
    };
    let midpoint = minimum * 0.5 + maximum * 0.5;
    let half_span = (maximum * 0.5 - minimum * 0.5) / zoom as f64;
    minimum = midpoint - half_span;
    maximum = midpoint + half_span;
    let y_position = |value: f64| {
        let fraction = if (maximum - minimum).is_finite() {
            (value - minimum) / (maximum - minimum)
        } else {
            (value * 0.5 - minimum * 0.5) / (maximum * 0.5 - minimum * 0.5)
        };
        y1 - fraction as f32 * (y1 - y0)
    };
    let draw = ui.get_window_draw_list();
    draw.add_text(
        [x0, origin[1] + 1.0],
        palette.text,
        format!("{} index", axis_name(axis_names, slice.x_axis)),
    );
    draw.add_line([x0, y0], [x0, y1], palette.text).build();
    draw.add_line([x0, y1], [x1, y1], palette.text).build();
    for step in 0..=4 {
        let fraction = step as f32 / 4.0;
        let y = y1 - fraction * (y1 - y0);
        let value = minimum * (1.0 - fraction as f64) + maximum * fraction as f64;
        draw.add_line([x0 - 4.0, y], [x1, y], palette.border)
            .build();
        let label = format_value(value, decimals);
        let width = ui.calc_text_size(&label)[0];
        draw.add_text(
            [x0 - width - 8.0, y - ui.text_line_height() * 0.5],
            palette.text,
            label,
        );
    }
    let visible_start = x_min.floor().max(0.0) as usize;
    let visible_end = (x_min + x_span).ceil().min(data.x_len as f32) as usize;
    let visible_count = visible_end.saturating_sub(visible_start).max(1);
    let x_ticks = visible_count
        .saturating_sub(1)
        .min(((x1 - x0) / (tick_width + 16.0).max(55.0)).floor().max(1.0) as usize)
        .min(5);
    for index in axis_ticks(visible_count, x_ticks + 1)
        .into_iter()
        .map(|index| visible_start + index)
    {
        let x = x0 + (index as f32 + 0.5 - x_min) / x_span * (x1 - x0);
        draw.add_line([x, y1], [x, y1 + 5.0], palette.text).build();
        let index_text = index.to_string();
        let width = ui.calc_text_size(&index_text)[0];
        draw.add_text([x - width * 0.5, y1 + 8.0], palette.text, index_text);
        if let Some(value) = x_labels
            .get(&index)
            .and_then(|label| tick_coordinate(index, label))
        {
            let width = ui.calc_text_size(value)[0];
            draw.add_text(
                [x - width * 0.5, y1 + 8.0 + line_height],
                palette.text,
                value,
            );
        }
    }
    draw.add_text([x0, y1 + 14.0 + line_height * 2.0], palette.text, "Index");
    let clip = draw.push_clip_rect([x0, y0], [x1, y1], true);
    for (line_number, line) in data.lines.iter().enumerate() {
        let color = profile_line_color(palette, line_number);
        let mut previous = None;
        for (bin_index, bin) in line.bins.iter().enumerate() {
            if bin.count == 0 {
                previous = None;
                continue;
            }
            let x = x_position(bin_index, line.bins.len());
            if previous.is_none() || line.bins.len() <= 256 {
                draw.add_circle([x, y_position(bin.first)], 2.0, color)
                    .filled(true)
                    .build();
            }
            if bin.count > 1 {
                draw.add_line([x, y_position(bin.min)], [x, y_position(bin.max)], color)
                    .thickness(1.5)
                    .build();
            }
            if let Some((old_x, old_value)) = previous {
                draw.add_line(
                    [old_x, y_position(old_value)],
                    [x, y_position(bin.first)],
                    color,
                )
                .thickness(1.8)
                .build();
            }
            previous = Some((x, bin.last));
        }
    }
    drop(clip);
}
