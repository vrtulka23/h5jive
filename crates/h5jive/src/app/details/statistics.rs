use super::super::*;

impl App {
    pub(crate) fn draw_statistics_tab(&mut self, ui: &Ui) {
        let Some(selection) = self.selection.clone() else {
            ui.text_wrapped("Select a dataset to see its statistics.");
            return;
        };
        let Some(entry) = find_entry(&self.files[selection.file].tree, &selection.path) else {
            return;
        };
        match &entry.kind {
            Kind::Group => {
                ui.text(format!("# Children: {}", entry.children.len()));
                return;
            }
            Kind::Other => {
                ui.text("No numeric statistics for this object.");
                return;
            }
            Kind::Dataset { .. } => {}
        }
        self.poll_statistics(&selection);
        if self.attributes_cache.is_none() {
            self.attributes_cache = Some(
                self.files[selection.file]
                    .attributes(&selection.path)
                    .map_err(|err| err.to_string()),
            );
        }
        let Some(result) = &self.statistics_cache else {
            ui.text("Calculating statistics...");
            return;
        };
        let statistics = match result {
            Ok(statistics) => statistics,
            Err(err) => {
                ui.text_wrapped(format!("Statistics unavailable: {err}"));
                return;
            }
        };
        if let Some(_table) = ui.begin_table_with_flags(
            "statistics_rows",
            2,
            TableOptions::new()
                .flags(
                    TableFlags::ROW_BG | TableFlags::BORDERS_INNER_H | TableFlags::NO_HOST_EXTEND_X,
                )
                .sizing_policy(TableSizingPolicy::FixedFit),
        ) {
            let width = ui.content_region_avail()[0].min(self.settings.details_max_width);
            let key_width = (width * 0.36).clamp(50.0, 108.0);
            ui.table_setup_column_fixed_width("Statistic", TableColumnFlags::NONE, key_width);
            ui.table_setup_column_fixed_width(
                "Value",
                TableColumnFlags::NONE,
                (width - key_width - 2.0 * PANEL_GAP).max(24.0),
            );
            detail_row(ui, "Shape", format!("{:?}", statistics.info.shape));
            detail_row(
                ui,
                "Byte size",
                format!(
                    "{} B ({})",
                    statistics.info.byte_size,
                    human_bytes(statistics.info.byte_size, self.settings.display_decimals)
                ),
            );
            detail_row(ui, "Datatype", &statistics.info.datatype);
            if let Some(attribute) = self
                .attributes_cache
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(|attributes| {
                    attributes
                        .iter()
                        .find(|attribute| attribute.name == "_FillValue")
                })
            {
                detail_row(
                    ui,
                    "Fill value",
                    format_attribute_value(attribute, self.settings.display_decimals),
                );
            }
            if statistics.info.numeric {
                detail_row(
                    ui,
                    "Min",
                    format_stat(statistics.min, self.settings.display_decimals),
                );
                detail_row(
                    ui,
                    "Max",
                    format_stat(statistics.max, self.settings.display_decimals),
                );
                detail_row(
                    ui,
                    "Mean",
                    format_stat(statistics.mean, self.settings.display_decimals),
                );
                detail_row(
                    ui,
                    "Std. deviation",
                    format_stat(
                        statistics.standard_deviation,
                        self.settings.display_decimals,
                    ),
                );
                detail_row(
                    ui,
                    "Variance",
                    format_stat(statistics.variance, self.settings.display_decimals),
                );
                let qualifier = if statistics.approximate_quartiles {
                    " (approx.)"
                } else {
                    ""
                };
                detail_row(
                    ui,
                    &format!("Q1{qualifier}"),
                    format_stat(statistics.first_quartile, self.settings.display_decimals),
                );
                detail_row(
                    ui,
                    "Q2 (Median)",
                    format_stat(statistics.median, self.settings.display_decimals),
                );
                detail_row(
                    ui,
                    &format!("Q3{qualifier}"),
                    format_stat(statistics.third_quartile, self.settings.display_decimals),
                );
                let total = statistics.info.elements;
                let count =
                    |value| format_count_percentage(value, total, self.settings.display_decimals);
                detail_row(ui, "# Values", count(total));
                detail_row(ui, "# Outliers", count(statistics.outliers));
                detail_row(ui, "# Positives", count(statistics.positive));
                detail_row(ui, "# Zeros", count(statistics.zero));
                detail_row(ui, "# Negatives", count(statistics.negative));
                detail_row_special(
                    ui,
                    "# NaN",
                    SpecialValue::Nan,
                    statistics.nan,
                    total,
                    self.settings.display_decimals,
                    &self.settings.palette,
                );
                detail_row_special(
                    ui,
                    "# -Inf",
                    SpecialValue::NegativeInf,
                    statistics.negative_infinity,
                    total,
                    self.settings.display_decimals,
                    &self.settings.palette,
                );
                detail_row_special(
                    ui,
                    "# +Inf",
                    SpecialValue::PositiveInf,
                    statistics.positive_infinity,
                    total,
                    self.settings.display_decimals,
                    &self.settings.palette,
                );
                detail_row_special(
                    ui,
                    "# Fills",
                    SpecialValue::Fill,
                    statistics.fill,
                    total,
                    self.settings.display_decimals,
                    &self.settings.palette,
                );
            } else {
                detail_row(
                    ui,
                    "# Values",
                    format_count_percentage(
                        statistics.info.elements,
                        statistics.info.elements,
                        self.settings.display_decimals,
                    ),
                );
            }
        }
        if statistics.approximate_quartiles {
            ui.text_wrapped("Quartiles use a bounded sample.");
        } else if statistics.info.numeric {
            ui.text_wrapped("Fill and nonfinite values are excluded from numeric statistics.");
        }
    }
}
