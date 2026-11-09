use super::super::*;

impl App {
    pub(crate) fn draw_coordinates_tab(&mut self, ui: &Ui) {
        let Some(selection) = self.selection.clone() else {
            ui.text_wrapped("Select a dataset to see its coordinates.");
            return;
        };
        let Some(entry) = find_entry(&self.files[selection.file].tree, &selection.path) else {
            return;
        };
        if !matches!(entry.kind, Kind::Dataset { .. }) {
            ui.text_wrapped("Select a dataset to see its coordinates.");
            return;
        }
        if self.coordinates_cache.is_none() {
            self.coordinates_cache = Some(
                self.files[selection.file]
                    .coordinate_axes(&selection.path)
                    .map_err(|err| err.to_string()),
            );
        }
        match self.coordinates_cache.as_ref().unwrap() {
            Ok(axes) if axes.is_empty() => ui.text("Scalar dataset: no dimensions"),
            Ok(axes) => {
                let (outer_size, scroll) = table_extent(
                    ui,
                    96.0 + axes.len() as f32 * 136.0,
                    7,
                    self.settings.table_scrollbar_padding,
                );
                let mut flags = TableFlags::BORDERS | TableFlags::ROW_BG | TableFlags::RESIZABLE;
                if scroll {
                    flags |= TableFlags::SCROLL_X | TableFlags::SCROLL_Y;
                } else {
                    flags |= TableFlags::NO_HOST_EXTEND_X;
                }
                if let Some(_table) = ui.begin_table_with_sizing(
                    "coordinate_dimensions",
                    axes.len() + 1,
                    TableOptions::new()
                        .flags(flags)
                        .sizing_policy(TableSizingPolicy::FixedFit),
                    outer_size,
                    0.0,
                ) {
                    ui.table_setup_column_fixed_width("Property", TableColumnFlags::NONE, 96.0);
                    for (index, _) in axes.iter().enumerate() {
                        ui.table_setup_column_fixed_width(
                            format!("Axis {index}"),
                            TableColumnFlags::NONE,
                            136.0,
                        );
                    }
                    if scroll {
                        ui.table_setup_scroll_freeze(1, 1);
                    }
                    ui.table_headers_row();
                    for property in 0..7 {
                        ui.table_next_row();
                        ui.table_next_column();
                        ui.text(
                            [
                                "Axis",
                                "Dimension",
                                "Length",
                                "Variable",
                                "Units",
                                "First",
                                "Last",
                            ][property],
                        );
                        for (index, axis) in axes.iter().enumerate() {
                            let value = match property {
                                0 => index.to_string(),
                                1 => axis.name.clone(),
                                2 => axis.length.to_string(),
                                3 => axis
                                    .variable
                                    .as_deref()
                                    .and_then(|path| path.rsplit('/').next())
                                    .unwrap_or("-")
                                    .to_owned(),
                                4 => axis.units.as_deref().unwrap_or("-").to_owned(),
                                5 => axis
                                    .first
                                    .map(|value| {
                                        format_value(value, self.settings.display_decimals)
                                    })
                                    .unwrap_or_else(|| "-".into()),
                                _ => axis
                                    .last
                                    .map(|value| {
                                        format_value(value, self.settings.display_decimals)
                                    })
                                    .unwrap_or_else(|| "-".into()),
                            };
                            ui.table_next_column();
                            ui.text(value);
                        }
                    }
                }
            }
            Err(err) => ui.text_wrapped(format!("Coordinates unavailable: {err}")),
        }
    }
}
