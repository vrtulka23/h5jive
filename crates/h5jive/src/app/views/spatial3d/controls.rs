use super::*;

impl VolumeProjection {
    pub(super) fn angles(self) -> (f32, f32) {
        match self {
            Self::Isometric => (0.65, 0.45),
            Self::Xy => (std::f32::consts::PI, std::f32::consts::PI),
            Self::Yz => (std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
            Self::Zx => (std::f32::consts::PI, -std::f32::consts::FRAC_PI_2),
        }
    }

    pub(super) fn axis_corner(self) -> [f32; 3] {
        match self {
            Self::Isometric | Self::Xy => [0.5, 0.5, 0.5],
            Self::Yz => [-0.5, 0.5, 0.5],
            Self::Zx => [0.5, -0.5, 0.5],
        }
    }
}

impl App {
    pub(crate) fn draw_volume_view(
        &mut self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        plot_ctx: &PlotContext,
    ) {
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
                ui.text("Select a 1D to 4D numeric dataset for the 3D preview.");
                return;
            }
        };
        if shape.contains(&0) {
            ui.text("Empty dataset");
            return;
        }
        let height = (ui.content_region_avail()[1] - self.volume_footer_height)
            .max(self.settings.panel_min_height);
        let origin = ui.cursor_screen_pos();
        self.view_rect = [origin[0], origin[1], ui.content_region_avail()[0], height];
        ui.child_window("volume_panel")
            .size([0.0, height])
            .flags(WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE)
            .build(ui, || {
                self.draw_volume_canvas(ui, plot_ui, plot_ctx, &selection, &shape);
            });
        let footer_start = ui.cursor_pos_y();
        ui.separator();
        let width = ui.content_region_avail()[0];
        let button_width = self.settings.update_button_width.min(width * 0.23);
        let special_width = (width * 0.15).clamp(110.0, 150.0);
        if let Some(_table) =
            begin_control_columns_with_special(ui, "volume_controls", special_width, button_width)
        {
            self.draw_volume_controls(ui, &shape);
            ui.table_next_column();
            let extra_width = ui.content_region_avail()[0].max(1.0);
            self.draw_volume_projection_buttons(ui, extra_width);
            if ui.button_with_size("Center object", [extra_width, 24.0]) {
                self.volume_pan = [0.0, 0.0];
            }
            ui.table_next_column();
            let pending = self.draft_volume != self.applied_volume
                || self.draft_volume_opacity != self.volume_opacity
                || self.draft_volume_log_scale != self.volume_log_scale;
            let special_height = 2.0 * 26.0 + 24.0 + 2.0 * ui.clone_style().item_spacing()[1];
            let update_height = self.settings.update_button_height.max(special_height + 4.0);
            if draw_update_button_with_height(ui, &self.settings, pending, update_height) {
                let slice_changed = self.applied_volume != self.draft_volume;
                self.applied_volume = self.draft_volume.clone();
                self.volume_opacity = self.draft_volume_opacity;
                self.volume_log_scale = self.draft_volume_log_scale;
                if slice_changed {
                    self.volume_cache = None;
                    self.volume_job = None;
                    self.volume_raster = None;
                }
                self.volume_raster_job = None;
            }
        }
        self.volume_footer_height = (ui.cursor_pos_y() - footer_start).max(1.0);
    }

    fn draw_volume_controls(&mut self, ui: &Ui, shape: &[usize]) {
        let Some(mut slice) = self.draft_volume.clone() else {
            return;
        };
        if (2..=3).contains(&shape.len()) {
            let mut hidden_enabled = slice.hidden.is_some();
            if ui.checkbox(
                "Pin a hidden axis##volume_hidden_toggle",
                &mut hidden_enabled,
            ) {
                slice.set_hidden(hidden_enabled, self.settings.pinned_default, shape);
            }
        }
        let visible_count = slice.visible_count();
        let labels = axis_option_labels(&self.axis_names, shape);
        let controls_width = ui.content_region_avail()[0];
        let select_width = (controls_width * 0.38).clamp(110.0, 180.0);
        if let Some(_table) = ui.begin_table("volume_axis_controls", 2) {
            ui.table_setup_column_fixed_width("Axis", TableColumnFlags::NONE, select_width);
            ui.table_setup_column_fixed_width(
                "Indexes",
                TableColumnFlags::NONE,
                (controls_width - select_width - PANEL_GAP).max(80.0),
            );
            for slot in 0..visible_count + usize::from(slice.hidden.is_some()) {
                let hidden_slot = slot == visible_count;
                let mut selected = if hidden_slot {
                    slice.hidden.unwrap()
                } else {
                    slice.axes[slot].unwrap()
                };
                ui.table_next_row();
                ui.table_next_column();
                ui.text(if hidden_slot {
                    "H"
                } else {
                    ["X", "Y", "Z"][slot]
                });
                ui.same_line();
                ui.set_next_item_width(ui.content_region_avail()[0].max(80.0));
                if ui.combo_simple_string(format!("##volume_axis_{slot}"), &mut selected, &labels) {
                    assign_volume_axis(&mut slice, slot, selected);
                }
                ui.table_next_column();
                if hidden_slot {
                    let hidden = slice.hidden.unwrap();
                    let max = shape[hidden].saturating_sub(1).min(i32::MAX as usize) as i32;
                    let mut index = slice.pinned.min(max as usize) as i32;
                    ui.set_next_item_width(ui.content_region_avail()[0].max(80.0));
                    ui.slider("##volume_pin", 0, max, &mut index);
                    slice.pinned = index as usize;
                } else {
                    let axis = slice.axes[slot].unwrap();
                    let max = shape[axis].saturating_sub(1).min(i32::MAX as usize) as i32;
                    let [start, end] = slice.ranges[axis];
                    let mut start = start.min(max as usize) as i32;
                    let mut end = end.min(max as usize) as i32;
                    ui.text(format!("{start}..{end}"));
                    ui.same_line();
                    self.draw_volume_range(ui, axis, max, &mut start, &mut end);
                    slice.ranges[axis] = [start as usize, end as usize];
                }
            }
        }
        self.draft_volume = Some(slice);
        let mut opacity_position = self.draft_volume_opacity.sqrt();
        let available = ui.content_region_avail()[0];
        let label_width = ui.calc_text_size("Opacity 0.000")[0];
        let log_width = ui.calc_text_size("Log scale")[0] + 24.0;
        ui.set_next_item_width((available - label_width - log_width - 24.0).max(80.0));
        ui.slider_config("##volume_opacity", 0.0, 1.0)
            .try_display_format(" ")
            .expect("valid opacity format")
            .build(&mut opacity_position);
        self.draft_volume_opacity = opacity_from_slider(opacity_position);
        ui.same_line();
        ui.text(format!("Opacity {:.3}", self.draft_volume_opacity));
        ui.same_line();
        ui.checkbox("Log scale", &mut self.draft_volume_log_scale);
        ui.set_item_tooltip("Signed logarithmic scaling keeps zero and negative values visible.");
    }

    fn draw_volume_projection_buttons(&mut self, ui: &Ui, width: f32) {
        let gap = CONTROL_INLINE_GAP;
        let cell_width = ((width - gap) * 0.5).max(1.0);
        for row in [
            [
                ("Iso", VolumeProjection::Isometric),
                ("XY", VolumeProjection::Xy),
            ],
            [("YZ", VolumeProjection::Yz), ("ZX", VolumeProjection::Zx)],
        ] {
            for (column, (label, projection)) in row.into_iter().enumerate() {
                if column > 0 {
                    ui.same_line_with_spacing(0.0, gap);
                }
                if ui.button_with_size(label, [cell_width, 26.0]) {
                    let (yaw, pitch) = projection.angles();
                    self.volume_rotation = VolumeRotation::from_angles(yaw, pitch);
                    self.volume_axis_corner = projection.axis_corner();
                    self.volume_projection = Some(projection);
                }
                if self.volume_projection == Some(projection) {
                    ui.get_window_draw_list()
                        .add_rect(
                            ui.item_rect_min(),
                            ui.item_rect_max(),
                            self.settings.palette.accent,
                        )
                        .rounding(ui.clone_style().frame_rounding())
                        .thickness(2.0)
                        .build();
                }
            }
        }
    }

    fn draw_volume_range(
        &mut self,
        ui: &Ui,
        axis: usize,
        max: i32,
        start: &mut i32,
        end: &mut i32,
    ) {
        use dear_app::imgui::MouseButton;

        if !ui.is_mouse_down(MouseButton::Left) {
            self.volume_range_drag = None;
        }
        let width = ui.content_region_avail()[0].max(80.0);
        ui.invisible_button(format!("##volume_range_{axis}"), [width, 22.0]);
        let min = ui.item_rect_min();
        let max_pos = ui.item_rect_max();
        let left = min[0] + 10.0;
        let right = max_pos[0] - 10.0;
        let y = (min[1] + max_pos[1]) * 0.5;
        let position = |index: i32| {
            if max == 0 {
                left
            } else {
                left + index as f32 / max as f32 * (right - left)
            }
        };
        let hovered = ui.is_item_hovered();
        if hovered && ui.is_mouse_clicked(MouseButton::Left) {
            let mouse = ui.mouse_pos()[0];
            let upper = if start == end {
                mouse >= position(*start)
            } else {
                (mouse - position(*end)).abs() < (mouse - position(*start)).abs()
            };
            self.volume_range_drag = Some((axis, upper));
        }
        if let Some((drag_axis, upper)) = self.volume_range_drag {
            if drag_axis == axis && ui.is_mouse_down(MouseButton::Left) {
                let fraction = ((ui.mouse_pos()[0] - left) / (right - left)).clamp(0.0, 1.0);
                let index = (fraction * max as f32).round() as i32;
                if upper {
                    *end = index.max(*start);
                } else {
                    *start = index.min(*end);
                }
            }
        }
        if hovered
            || self
                .volume_range_drag
                .is_some_and(|(drag_axis, _)| drag_axis == axis)
        {
            ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
        }
        if hovered {
            ui.set_item_tooltip(format!("Visible indexes: {start} through {end}"));
        }
        let control_bg = ui.style_color(StyleColor::FrameBg);
        let grab = ui.style_color(StyleColor::SliderGrab);
        let grab_active = ui.style_color(StyleColor::SliderGrabActive);
        let accent = self.settings.palette.accent;
        let track_rounding = ui.clone_style().frame_rounding();
        let grab_rounding = ui.clone_style().grab_rounding();
        let draw = ui.get_window_draw_list();
        draw.add_rect([left, y - 3.0], [right, y + 3.0], control_bg)
            .filled(true)
            .rounding(track_rounding)
            .build();
        if start < end {
            draw.add_rect(
                [position(*start), y - 3.0],
                [position(*end), y + 3.0],
                accent,
            )
            .filled(true)
            .rounding(track_rounding)
            .build();
        }
        for (index, upper) in [(*start, false), (*end, true)] {
            let x = position(index);
            let active = self.volume_range_drag == Some((axis, upper));
            draw.add_rect(
                [x - 5.0, y - 9.0],
                [x + 5.0, y + 9.0],
                if active { grab_active } else { grab },
            )
            .filled(true)
            .rounding(grab_rounding)
            .build();
        }
    }
}
