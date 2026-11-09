use super::super::*;

impl App {
    pub(crate) fn draw_data_view(&mut self, ui: &Ui, plot_ui: &PlotUi<'_>, plot_ctx: &PlotContext) {
        let Some(selection) = self.selection.clone() else {
            ui.text("Select a dataset from the tree.");
            return;
        };
        let file = &self.files[selection.file];
        let Some(entry) = find_entry(&file.tree, &selection.path) else {
            return;
        };
        let shape = match &entry.kind {
            Kind::Dataset {
                shape,
                numeric: true,
            } => shape.clone(),
            Kind::Dataset { .. } => {
                ui.text("Non-numeric dataset");
                return;
            }
            Kind::Group => {
                ui.text(format!("Group: {} children", entry.children.len()));
                return;
            }
            Kind::Other => {
                ui.text("Other HDF5 object");
                return;
            }
        };
        if !(1..=4).contains(&shape.len()) || self.view == ViewMode::Slice2d && shape.len() < 2 {
            ui.text(if self.view == ViewMode::Slice2d {
                "Select a 2D, 3D, or 4D numeric dataset for the heatmap."
            } else {
                "The data table supports 1D to 4D datasets."
            });
            return;
        }
        let footer_height = if self.view == ViewMode::Slice2d {
            self.slice_footer_height
        } else {
            self.table_footer_height
        };
        let height =
            (ui.content_region_avail()[1] - footer_height).max(self.settings.panel_min_height);
        let origin = ui.cursor_screen_pos();
        self.view_rect = [origin[0], origin[1], ui.content_region_avail()[0], height];
        let panel_id = if self.view == ViewMode::Slice2d {
            "heatmap_panel"
        } else {
            "data_table_panel"
        };
        let panel_flags = if self.view == ViewMode::Slice2d {
            WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE
        } else {
            WindowFlags::empty()
        };
        ui.child_window(panel_id)
            .size([0.0, height])
            .flags(panel_flags)
            .build(ui, || {
                if self.view == ViewMode::Slice2d {
                    self.draw_heatmap(ui, plot_ui, plot_ctx, &selection, &shape);
                } else {
                    self.draw_value_table(ui, &selection, &shape);
                }
            });
        let footer_start = ui.cursor_pos_y();
        ui.separator();
        let button_width = control_action_width(ui, &self.settings);
        if let Some(_table) = begin_control_columns(ui, "view_controls", button_width) {
            self.draw_dimension_controls(ui, &shape);
            ui.table_next_column();
            {
                let pending = if self.view == ViewMode::Slice2d {
                    self.draft_slice != self.applied_slice
                } else {
                    self.draft_table != self.applied_table
                };
                if draw_update_button(ui, &self.settings, pending) {
                    if self.view == ViewMode::Slice2d {
                        self.applied_slice = self.draft_slice.clone();
                    } else {
                        self.applied_table = self.draft_table.clone();
                    }
                    self.region_cache.clear();
                    self.heatmap_cache = None;
                }
            }
        }
        let measured = (ui.cursor_pos_y() - footer_start).max(1.0);
        if self.view == ViewMode::Slice2d {
            self.slice_footer_height = measured;
        } else {
            self.table_footer_height = measured;
        }
    }
    pub(crate) fn draw_dimension_controls(&mut self, ui: &Ui, shape: &[usize]) {
        let pinned = if self.view == ViewMode::Slice2d {
            self.draft_slice.as_ref()
        } else {
            self.draft_table.as_ref()
        }
        .map(|draft| draft.pinned.clone());
        let coordinate_labels = pinned
            .as_ref()
            .map(|pinned| self.pinned_coordinate_labels(pinned));
        let draft = if self.view == ViewMode::Slice2d {
            &mut self.draft_slice
        } else {
            &mut self.draft_table
        };
        let Some(slice) = draft.as_mut() else {
            return;
        };
        let labels = axis_option_labels(&self.axis_names, shape);
        let available_width = ui.content_region_avail()[0];
        let combo_width = if slice.row_axis.is_some() {
            ((available_width
                - ui.calc_text_size("Rows")[0]
                - ui.calc_text_size("Columns")[0]
                - 32.0)
                / 2.0)
                .max(60.0)
        } else {
            (available_width - ui.calc_text_size("Column")[0] - 12.0).max(60.0)
        };
        if let Some(mut row) = slice.row_axis {
            ui.text("Rows");
            ui.same_line();
            ui.set_next_item_width(combo_width);
            if ui.combo_simple_string("##row_axis", &mut row, &labels) {
                slice.row_axis = Some(row);
                if row == slice.column_axis {
                    slice.column_axis = (0..shape.len()).find(|&axis| axis != row).unwrap();
                }
            }
            ui.same_line();
        }
        let column_label = if slice.row_axis.is_none() {
            "Column"
        } else {
            "Columns"
        };
        ui.text(column_label);
        ui.same_line();
        ui.set_next_item_width(combo_width);
        if ui.combo_simple_string("##column_axis", &mut slice.column_axis, &labels)
            && slice.row_axis == Some(slice.column_axis)
        {
            slice.row_axis = (0..shape.len()).find(|&axis| axis != slice.column_axis);
        }
        for (axis, &size) in shape.iter().enumerate() {
            if slice.row_axis == Some(axis) || slice.column_axis == axis {
                continue;
            }
            let mut index = slice.pinned[axis] as i32;
            ui.set_next_item_width((ui.content_region_avail()[0] * 0.65).max(60.0));
            let name = axis_name(&self.axis_names, axis);
            ui.slider(
                format!(
                    "{name} {}##pinned_axis_{axis}",
                    coordinate_labels
                        .as_ref()
                        .map_or("", |labels| labels[axis].as_str())
                ),
                0,
                size.saturating_sub(1).min(i32::MAX as usize) as i32,
                &mut index,
            );
            slice.pinned[axis] = index as usize;
        }
    }
    pub(crate) fn cached_region(&mut self, key: &RegionKey) -> &Result<Region, String> {
        if !self.region_cache.iter().any(|(cached, _)| cached == key) {
            let result = self.files[key.file]
                .region(&key.path, &key.start, &key.count)
                .map_err(|err| err.to_string());
            if self.region_cache.len() >= self.settings.region_cache_entries {
                let _ = self.region_cache.remove(0);
            }
            self.region_cache.push((key.clone(), result));
        }
        &self
            .region_cache
            .iter()
            .find(|(cached, _)| cached == key)
            .unwrap()
            .1
    }
    pub(crate) fn draw_value_table(&mut self, ui: &Ui, selection: &Selection, shape: &[usize]) {
        let Some(slice) = self.applied_table.clone() else {
            return;
        };
        if shape.iter().any(|&size| size == 0) {
            ui.text("Empty dataset");
            return;
        }
        let rows = slice.rows(shape);
        if rows >= i32::MAX as usize {
            ui.text("This dimension is too long for the table scrollbar.");
            return;
        }
        let columns = slice.columns(shape);
        let page_columns_limit = self.settings.table_columns_per_page;
        let pages = columns.div_ceil(page_columns_limit);
        self.table_column_page = self.table_column_page.min(pages - 1);
        if pages > 1 {
            if ui.small_button("Previous columns") {
                self.table_column_page = self.table_column_page.saturating_sub(1);
                self.region_cache.clear();
            }
            ui.same_line();
            ui.text(format!(
                "Columns page {} / {}",
                self.table_column_page + 1,
                pages
            ));
            ui.same_line();
            if ui.small_button("Next columns") {
                self.table_column_page = (self.table_column_page + 1).min(pages - 1);
                self.region_cache.clear();
            }
        }
        let column_start = self.table_column_page * page_columns_limit;
        let page_columns = (columns - column_start).min(page_columns_limit);
        let axes = self.coordinate_axes_for(selection);
        let row_axis = slice.row_axis.unwrap_or(slice.column_axis);
        let row_has_values = axes
            .as_ref()
            .and_then(|axes| axes.get(row_axis))
            .is_some_and(|axis| axis.variable.is_some());
        let column_has_values = slice.row_axis.is_some()
            && axes
                .as_ref()
                .and_then(|axes| axes.get(slice.column_axis))
                .is_some_and(|axis| axis.variable.is_some());
        let column_labels = if slice.row_axis.is_some() {
            self.coordinate_labels(selection, slice.column_axis, column_start, page_columns)
        } else {
            vec!["0".into()]
        };
        let row_width = if row_has_values {
            let first = self.coordinate_label(selection, row_axis, 0);
            let last = self.coordinate_label(selection, row_axis, rows - 1);
            (ui.calc_text_size(&first)[0].max(ui.calc_text_size(&last)[0]) + 24.0)
                .max(self.settings.table_row_header_width)
        } else {
            self.settings.table_row_header_width
        };
        let column_widths = column_labels
            .iter()
            .map(|label| {
                if column_has_values {
                    (ui.calc_text_size(label)[0] + 24.0).max(self.settings.table_value_column_width)
                } else {
                    self.settings.table_value_column_width
                }
            })
            .collect::<Vec<_>>();
        let (outer_size, scroll) = table_extent(
            ui,
            row_width
                + 2.0 * TABLE_CELL_PADDING[0]
                + column_widths.iter().sum::<f32>()
                + page_columns as f32 * 2.0 * TABLE_CELL_PADDING[0],
            rows,
            self.settings.table_scrollbar_padding,
        );
        let mut flags = TableFlags::BORDERS | TableFlags::ROW_BG;
        if scroll {
            flags |= TableFlags::SCROLL_X | TableFlags::SCROLL_Y;
        } else {
            flags |= TableFlags::NO_HOST_EXTEND_X;
        }
        if let Some(_table) = ui.begin_table_with_sizing(
            format!(
                "values##{}:{}:{}:{:?}:{}",
                selection.file, selection.path, slice.column_axis, slice.row_axis, column_start
            ),
            page_columns + 1,
            TableOptions::new()
                .flags(flags)
                .sizing_policy(TableSizingPolicy::FixedFit),
            outer_size,
            0.0,
        ) {
            ui.table_setup_column_fixed_width("Row", TableColumnFlags::NONE, row_width);
            for (label, width) in column_labels.into_iter().zip(column_widths) {
                ui.table_setup_column_fixed_width(label, TableColumnFlags::NONE, width);
            }
            if scroll {
                ui.table_setup_scroll_freeze(1, 1);
            }
            ui.table_headers_row();
            let visible: Vec<usize> = (0..page_columns)
                .filter(|&column| {
                    ui.table_get_column_flags(column + 1)
                        .contains(TableColumnStateFlags::IS_VISIBLE)
                })
                .collect();
            let first_column = visible.first().copied().unwrap_or(0) + column_start;
            let last_column = visible.last().copied().unwrap_or(0) + column_start + 1;
            let mut clipper = ListClipper::new(rows).begin(ui);
            while clipper.step() {
                let visible_rows = clipper.display_range();
                if visible_rows.is_empty() {
                    continue;
                }
                let (start, count) = slice.region(
                    shape,
                    visible_rows.start,
                    visible_rows.end,
                    first_column,
                    last_column,
                );
                let row_labels = self.coordinate_labels(
                    selection,
                    row_axis,
                    visible_rows.start,
                    visible_rows.end - visible_rows.start,
                );
                let key = RegionKey {
                    file: selection.file,
                    path: selection.path.clone(),
                    start,
                    count,
                };
                let palette = self.settings.palette.clone();
                let decimals = self.settings.display_decimals;
                let region = self.cached_region(&key);
                for (offset, row) in visible_rows.enumerate() {
                    ui.table_next_row();
                    ui.table_next_column();
                    ui.text(&row_labels[offset]);
                    for column in 0..page_columns {
                        if ui.table_set_column_index(column + 1) {
                            let actual_column = column_start + column;
                            match region {
                                Ok(region) => {
                                    let index = slice.region_offset(
                                        &region.shape,
                                        row,
                                        actual_column,
                                        &key.start,
                                    );
                                    if let Some(value) = region.data.get(index) {
                                        if let Some(special) =
                                            SpecialValue::from_value(*value, region.fill_value)
                                        {
                                            ui.text_colored(
                                                special.color(&palette),
                                                special.label(),
                                            );
                                        } else {
                                            ui.text(format_value(*value, decimals));
                                        }
                                    }
                                }
                                Err(err) => ui.text(err),
                            }
                        }
                    }
                }
            }
        }
    }
}
