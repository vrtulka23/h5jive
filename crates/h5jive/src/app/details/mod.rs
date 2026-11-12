use super::*;

mod attributes;
mod coordinates;
mod distribution;
mod statistics;

impl App {
    pub(crate) fn draw_details_panel(&mut self, ui: &Ui, height: f32) {
        ui.child_window("selection_details")
            .size([0.0, height])
            .flags(WindowFlags::NO_SCROLLBAR)
            .build(ui, || {
                let _content_padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
                if let Some(_tabs) = ui.tab_bar("details_tabs") {
                    let flags = if self.details_default_pending {
                        TabItemFlags::SET_SELECTED
                    } else {
                        TabItemFlags::NONE
                    };
                    if let Some(_tab) = ui.tab_item_with_flags("Attributes", None, flags) {
                        ui.child_window("attributes_content")
                            .size([0.0, 0.0])
                            .build(ui, || self.draw_attributes_tab(ui));
                    }
                    self.details_default_pending = false;
                    if let Some(_tab) = ui.tab_item("Statistics") {
                        ui.child_window("statistics_content")
                            .size([0.0, 0.0])
                            .build(ui, || {
                                self.draw_statistics_tab(ui);
                            });
                    }
                    if let Some(_tab) = ui.tab_item("Distribution") {
                        ui.child_window("distribution_content")
                            .size([0.0, 0.0])
                            .build(ui, || self.draw_distribution_tab(ui));
                    }
                    if let Some(_tab) = ui.tab_item("Coordinates") {
                        ui.child_window("coordinates_content")
                            .size([0.0, 0.0])
                            .build(ui, || self.draw_coordinates_tab(ui));
                    }
                }
            });
    }
}
