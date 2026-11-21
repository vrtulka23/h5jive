//! Shared attached path entry and browse control for every filesystem path.
use super::*;

/// Returns true when the attached browse button is pressed.
pub(super) fn file_path_field(ui: &Ui, id: &str, path: &mut String, width: f32) -> bool {
    let button = ui.frame_height();
    let gap = 8.0;
    let width = width.max(button + gap + 80.0);
    ui.set_next_item_width(width - button - gap);
    ui.input_text(format!("##path_{id}"), path).build();
    ui.same_line_with_spacing(0.0, gap);
    let clicked = ui.button_with_size(format!("##browse_{id}"), [button, button]);
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let color = ui.style_color(StyleColor::Text);
    let draw = ui.get_window_draw_list();
    let x = (min[0] + max[0]) * 0.5;
    let y = (min[1] + max[1]) * 0.5;
    draw.add_rect([x - 7.0, y - 3.0], [x + 7.0, y + 5.0], color)
        .thickness(1.5)
        .build();
    draw.add_line([x - 6.0, y - 5.0], [x - 1.0, y - 5.0], color)
        .thickness(1.5)
        .build();
    draw.add_line([x - 1.0, y - 5.0], [x + 1.0, y - 3.0], color)
        .thickness(1.5)
        .build();
    clicked
}
