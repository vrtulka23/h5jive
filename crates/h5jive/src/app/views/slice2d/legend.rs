use super::super::super::*;

pub(crate) fn draw_heatmap_legend(
    ui: &Ui,
    plot_ctx: &PlotContext,
    origin: [f32; 2],
    size: [f32; 2],
    range: Option<(f64, f64)>,
    pending: bool,
    preset: ColormapPreset,
    palette: &Palette,
    decimals: usize,
) {
    let draw = ui.get_window_draw_list();
    let text_color = palette.text;
    draw.add_text(
        [origin[0], origin[1] + 4.0],
        text_color,
        if pending {
            "Visible range"
        } else {
            "Data values"
        },
    );
    let bar_top = origin[1] + 24.0;
    let bar_height = (size[1] - 48.0).max(1.0);
    if let Some((min, max)) = range {
        let bar_x = origin[0] + 2.0;
        let bar_width = 22.0;
        let steps = (bar_height.ceil() as usize).clamp(1, 256);
        for step in 0..steps {
            let y = bar_top + bar_height * step as f32 / steps as f32;
            let next_y = bar_top + bar_height * (step + 1) as f32 / steps as f32;
            let fraction = 1.0 - (step as f32 + 0.5) / steps as f32;
            draw.add_rect(
                [bar_x, y],
                [bar_x + bar_width, next_y + 0.5],
                continuous_color(plot_ctx, fraction, preset, palette),
            )
            .filled(true)
            .build();
        }
        draw.add_rect(
            [bar_x, bar_top],
            [bar_x + bar_width, bar_top + bar_height],
            text_color,
        )
        .build();
        let label_x = bar_x + bar_width + 8.0;
        let midpoint = min * 0.5 + max * 0.5;
        for (y, value) in [
            (bar_top, max),
            (bar_top + bar_height * 0.5, midpoint),
            (bar_top + bar_height, min),
        ] {
            let label = format_value(value, decimals);
            draw.add_text(
                [label_x, y - ui.calc_text_size(&label)[1] * 0.5],
                text_color,
                label,
            );
        }
    } else {
        draw.add_text(
            [origin[0], bar_top],
            text_color,
            if pending {
                "Scanning range..."
            } else {
                "No finite values"
            },
        );
    }
}
