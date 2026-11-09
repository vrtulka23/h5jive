use super::super::*;

pub(crate) fn continuous_color(
    plot_ctx: &PlotContext,
    fraction: f32,
    preset: ColormapPreset,
    palette: &Palette,
) -> [f32; 4] {
    let fraction = fraction.clamp(0.0, 1.0);
    if preset == ColormapPreset::Viridis {
        return plot_ctx.sample_colormap_from(fraction, Colormap::Viridis);
    }
    let (low, high, position) = if fraction < 0.5 {
        (palette.heatmap_low, palette.heatmap_mid, fraction * 2.0)
    } else {
        (
            palette.heatmap_mid,
            palette.heatmap_high,
            (fraction - 0.5) * 2.0,
        )
    };
    std::array::from_fn(|channel| low[channel] + (high[channel] - low[channel]) * position)
}
