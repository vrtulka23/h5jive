use super::super::*;

impl App {
    pub(crate) fn draw_attributes_tab(&mut self, ui: &Ui) {
        let Some(selection) = self.selection.clone() else {
            ui.text_wrapped("Select a group or dataset to see its attributes.");
            return;
        };
        if self.attributes_cache.is_none() {
            self.attributes_cache = Some(
                self.files[selection.file]
                    .attributes(&selection.path)
                    .map_err(|err| err.to_string()),
            );
        }
        let Some(result) = &self.attributes_cache else {
            return;
        };
        let attributes = match result {
            Ok(attributes) => attributes,
            Err(err) => {
                ui.text_wrapped(format!("Attributes unavailable: {err}"));
                return;
            }
        };
        if attributes.is_empty() {
            ui.text("No attributes");
        } else if let Some(_table) = ui.begin_table_with_flags(
            "attribute_rows",
            2,
            TableOptions::new()
                .flags(
                    TableFlags::ROW_BG | TableFlags::BORDERS_INNER_H | TableFlags::NO_HOST_EXTEND_X,
                )
                .sizing_policy(TableSizingPolicy::FixedFit),
        ) {
            let width = ui.content_region_avail()[0].min(self.settings.details_max_width);
            let key_width = (width * 0.36).clamp(50.0, 108.0);
            ui.table_setup_column_fixed_width("Attribute", TableColumnFlags::NONE, key_width);
            ui.table_setup_column_fixed_width(
                "Value",
                TableColumnFlags::NONE,
                (width - key_width - 2.0 * PANEL_GAP).max(24.0),
            );
            for attribute in attributes {
                detail_row(
                    ui,
                    &attribute.name,
                    format_attribute_value(attribute, self.settings.display_decimals),
                );
            }
        }
    }
}
