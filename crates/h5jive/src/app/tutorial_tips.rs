use super::*;

const TOOLTIP_PADDING: [f32; 2] = [12.0, 10.0];
const TIP_SECTION_GAP: f32 = 10.0;

fn gap(ui: &Ui) {
    ui.set_cursor_pos([ui.cursor_pos_x(), ui.cursor_pos_y() + TIP_SECTION_GAP]);
}

/// Show plain content using the same inset as tutorial tips.
pub(super) fn show_plain_tip(ui: &Ui, text: &str) {
    let _padding = ui.push_style_var(StyleVar::WindowPadding(TOOLTIP_PADDING));
    ui.tooltip(|| ui.text(text));
}

pub(super) fn item_plain_tip(ui: &Ui, text: &str) {
    if ui.is_item_hovered() {
        show_plain_tip(ui, text);
    }
}

/// Shared tooltip behavior for the app and built-in plugins.
pub(super) fn item_tip(ui: &Ui, tutorial_tips: bool, normal: &str, tutorial: &str) {
    if tutorial_tips && !tutorial.is_empty() {
        if ui.is_item_hovered() {
            let _padding = ui.push_style_var(StyleVar::WindowPadding(TOOLTIP_PADDING));
            ui.tooltip(|| {
                if !normal.is_empty() {
                    ui.text(normal);
                    gap(ui);
                }
                ui.text(tutorial);
                gap(ui);
                ui.text_colored(
                    ui.style_color(StyleColor::TextDisabled),
                    "Click \"Help / Tutorial tips\" to turn off",
                );
            });
        }
    } else if !normal.is_empty() {
        item_plain_tip(ui, normal);
    }
}
