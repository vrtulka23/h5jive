use super::*;

impl ValueVisibility {
    pub(super) fn draw_panel(&mut self, ui: &Ui, palette: &Palette, width: f32) -> bool {
        let mut changed = false;
        let origin = ui.cursor_screen_pos();
        let spacing = ui.clone_style().item_spacing();
        let button_width = ((width - spacing[0]) * 0.5).max(1.0);
        let row_height = 26.0 + spacing[1];
        let neutral = palette.muted;
        changed |= toggle_button(
            ui,
            &mut self.positive,
            "+",
            "Positive values",
            neutral,
            button_width,
        );
        ui.same_line();
        changed |= toggle_button(
            ui,
            &mut self.negative,
            "-",
            "Negative values",
            neutral,
            button_width,
        );
        ui.set_cursor_screen_pos([origin[0], origin[1] + row_height]);
        changed |= toggle_button(
            ui,
            &mut self.zero,
            "0",
            "Zero values",
            neutral,
            button_width,
        );
        ui.same_line();
        let all_background = mix_color(ui.style_color(StyleColor::FrameBg), neutral, 0.38);
        let _all_button = ui.push_style_color(StyleColor::Button, all_background);
        let _all_hovered = ui.push_style_color(
            StyleColor::ButtonHovered,
            mix_color(all_background, neutral, 0.45),
        );
        let _all_active = ui.push_style_color(
            StyleColor::ButtonActive,
            mix_color(all_background, neutral, 0.65),
        );
        let all_enabled = self.all_enabled();
        let label = if all_enabled { "None" } else { "All" };
        if ui.button_with_size(label, [button_width, 26.0]) {
            self.set_all(!all_enabled);
            changed = true;
        }
        ui.set_item_tooltip(if all_enabled {
            "Hide all value classes"
        } else {
            "Show all value classes"
        });
        drop(_all_active);
        drop(_all_hovered);
        drop(_all_button);
        ui.set_cursor_screen_pos([origin[0], origin[1] + 2.0 * row_height]);
        changed |= toggle_button(
            ui,
            &mut self.nan,
            "NaN",
            "Not-a-number values",
            palette.nan,
            button_width,
        );
        ui.same_line();
        changed |= toggle_button(
            ui,
            &mut self.fill,
            "Fill",
            "Fill values",
            palette.fill,
            button_width,
        );
        ui.set_cursor_screen_pos([origin[0], origin[1] + 3.0 * row_height]);
        changed |= toggle_button(
            ui,
            &mut self.positive_inf,
            "+Inf",
            "Positive infinity values",
            palette.positive_inf,
            button_width,
        );
        ui.same_line();
        changed |= toggle_button(
            ui,
            &mut self.negative_inf,
            "-Inf",
            "Negative infinity values",
            palette.negative_inf,
            button_width,
        );
        changed
    }
}

fn toggle_button(
    ui: &Ui,
    enabled: &mut bool,
    label: &str,
    tooltip: &str,
    color: [f32; 4],
    width: f32,
) -> bool {
    let background = if *enabled {
        mix_color(ui.style_color(StyleColor::FrameBg), color, 0.38)
    } else {
        ui.style_color(StyleColor::FrameBg)
    };
    let _normal = ui.push_style_color(StyleColor::Button, background);
    let _hovered = ui.push_style_color(
        StyleColor::ButtonHovered,
        mix_color(background, color, 0.45),
    );
    let _active = ui.push_style_color(StyleColor::ButtonActive, mix_color(background, color, 0.65));
    let changed = ui.button_with_size(label, [width, 26.0]);
    ui.set_item_tooltip(format!(
        "{tooltip}: {}",
        if *enabled { "shown" } else { "hidden" }
    ));
    if changed {
        *enabled = !*enabled;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    #[test]
    fn bottom_visibility_row_stays_in_the_right_rail() {
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([700.0, 450.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let settings = Settings::load().unwrap();
        let mut visibility = ValueVisibility::default();
        let ui = context.frame();
        ui.window("Right rail")
            .size([600.0, 350.0], Condition::Always)
            .build(|| {
                let origin = ui.cursor_screen_pos();
                let rail_x = origin[0] + ui.content_region_avail()[0] - 126.0;
                ui.set_cursor_screen_pos([rail_x, origin[1]]);
                visibility.draw_panel(ui, &settings.palette, 126.0);
                let last_button = ui.item_rect_min();
                assert!(last_button[0] > rail_x + 60.0);
                assert!(last_button[0] < rail_x + 126.0);
            });
    }
}
