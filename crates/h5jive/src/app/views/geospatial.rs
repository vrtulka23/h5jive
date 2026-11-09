use super::super::*;

pub(crate) fn geo_action_width(ui: &Ui, settings: &Settings) -> f32 {
    let width = ui.content_region_avail()[0];
    settings.update_button_width.min(width * 0.23)
}

const GEO_SPECIAL_BUTTON_HEIGHT: f32 = 24.0;
const GEO_SPECIAL_BUTTON_WIDTH: f32 = 40.0;

fn geo_special_width(has_scan: bool) -> f32 {
    if has_scan {
        2.0 * GEO_SPECIAL_BUTTON_WIDTH + PANEL_GAP + 2.0 * TABLE_CELL_PADDING[0]
    } else {
        GEO_SPECIAL_BUTTON_WIDTH + 2.0 * TABLE_CELL_PADDING[0]
    }
}

fn geo_update_height(ui: &Ui, settings: &Settings) -> f32 {
    settings
        .update_button_height
        .max(3.0 * GEO_SPECIAL_BUTTON_HEIGHT + 2.0 * ui.clone_style().item_spacing()[1] + 4.0)
}

pub(crate) fn geo_controls_height(
    ui: &Ui,
    settings: &Settings,
    rank: usize,
    has_scan: bool,
) -> f32 {
    let width = ui.content_region_avail()[0];
    let controls_width =
        width - geo_special_width(has_scan) - geo_action_width(ui, settings) - 2.0 * PANEL_GAP;
    let pics_rows = if controls_width < 350.0 {
        3
    } else if controls_width < 480.0 {
        2
    } else {
        1
    };
    let rows = 1 + pics_rows + usize::from(rank >= 3) + usize::from(rank == 4);
    let fields = rows as f32 * ui.frame_height_with_spacing();
    let actions = geo_update_height(ui, settings);
    fields.max(actions) + 2.0 * TABLE_CELL_PADDING[1] + PANEL_GAP
}

pub(crate) fn draw_scan_filter_controls(
    ui: &Ui,
    id: &str,
    filter: &mut ScanFilter,
    palette: &Palette,
    width: f32,
) {
    let buttons = [
        (ScanFilter::Both, "both", "Show both scan directions"),
        (ScanFilter::Forward, "forward", "Show forward scan pixels"),
        (
            ScanFilter::Backward,
            "backward",
            "Show backward scan pixels",
        ),
    ];
    for (choice, name, tooltip) in buttons {
        let clicked = ui.invisible_button(
            format!("scan_{id}_{name}"),
            [width, GEO_SPECIAL_BUTTON_HEIGHT],
        );
        let min = ui.item_rect_min();
        let max = ui.item_rect_max();
        let hovered = ui.is_item_hovered();
        let selected = *filter == choice;
        let background = if selected {
            mix_color(palette.panel, palette.accent, 0.28)
        } else if hovered {
            palette.view_hover
        } else {
            palette.panel
        };
        let draw = ui.get_window_draw_list();
        draw_control_surface(
            &draw,
            min,
            max,
            background,
            if selected {
                palette.accent
            } else {
                palette.border
            },
            ui.clone_style().frame_rounding(),
        );
        let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
        let arrows: &[(f32, f32)] = match choice {
            ScanFilter::Both => &[(1.0, -3.5), (-1.0, 3.5)],
            ScanFilter::Forward => &[(1.0, 0.0)],
            ScanFilter::Backward => &[(-1.0, 0.0)],
        };
        for &(direction, offset_y) in arrows {
            let y = center[1] + offset_y;
            let tail = [center[0] - direction * 6.0, y];
            let tip = [center[0] + direction * 6.0, y];
            let wing_x = tip[0] - direction * 3.5;
            for (from, to) in [
                (tail, tip),
                (tip, [wing_x, y - 3.0]),
                (tip, [wing_x, y + 3.0]),
            ] {
                draw.add_line(from, to, palette.text).thickness(1.5).build();
            }
        }
        drop(draw);
        if clicked {
            *filter = choice;
        }
        ui.set_item_tooltip(tooltip);
    }
}

fn pixel_style_button(
    ui: &Ui,
    id: &str,
    style: usize,
    selected: bool,
    enabled: bool,
    palette: &Palette,
    width: f32,
) -> bool {
    let clicked = ui.invisible_button(format!("{id}_{style}"), [width, GEO_SPECIAL_BUTTON_HEIGHT]);
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let hovered = ui.is_item_hovered();
    let foreground = if enabled { palette.text } else { palette.muted };
    let background = if selected {
        mix_color(palette.panel, palette.accent, 0.28)
    } else if hovered && enabled {
        palette.view_hover
    } else {
        palette.panel
    };
    let draw = ui.get_window_draw_list();
    draw_control_surface(
        &draw,
        min,
        max,
        background,
        if selected {
            palette.accent
        } else {
            palette.border
        },
        ui.clone_style().frame_rounding(),
    );
    let icon = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    match style {
        0 => {
            for offset in [[-5.0, 3.0], [0.0, -4.0], [5.0, 3.0]] {
                draw.add_circle([icon[0] + offset[0], icon[1] + offset[1]], 2.0, foreground)
                    .filled(true)
                    .build();
            }
        }
        1 => {
            draw.add_rect(
                [icon[0] - 7.0, icon[1] - 5.0],
                [icon[0] + 7.0, icon[1] + 5.0],
                foreground,
            )
            .build();
        }
        _ => {
            draw.add_rect(
                [icon[0] - 7.0, icon[1] - 5.0],
                [icon[0] + 7.0, icon[1] + 5.0],
                foreground,
            )
            .filled(true)
            .build();
        }
    }
    drop(draw);
    if hovered {
        ui.set_item_tooltip(if enabled {
            [
                "Show ground pixel centers",
                "Show ground pixel edges",
                "Show filled ground pixel footprints",
            ][style]
        } else {
            "Pixel edges and fills require latitude/longitude bounds."
        });
    }
    clicked && enabled
}

/// Shared layer toggles used by the flat map and globe geospatial views.
pub(crate) fn draw_layer_controls(
    ui: &Ui,
    id: &str,
    dummy_color: &mut bool,
    dummy_rgb: &mut [f32; 3],
    data_opacity: &mut f32,
    palette: &Palette,
) {
    let _dummy_button_color = (*dummy_color).then(|| {
        ui.push_style_color(
            StyleColor::Button,
            mix_color(palette.panel, palette.accent, 0.32),
        )
    });
    if ui.button(format!("Dummy color##{id}")) {
        *dummy_color = !*dummy_color;
    }
    drop(_dummy_button_color);
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    ui.color_edit3_config(format!("##dummy_picker_{id}"), dummy_rgb)
        .flags(ColorEditFlags::NO_INPUTS | ColorEditFlags::NO_LABEL)
        .build();
    ui.set_item_tooltip(
        "Choose the color used for ordinary satellite pixels when Dummy color is on.",
    );
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    ui.text("Data opacity");
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
    let mut opacity_percent = (*data_opacity * 100.0).round() as i32;
    if ui.slider(format!("##data_opacity_{id}"), 0, 100, &mut opacity_percent) {
        *data_opacity = opacity_percent as f32 / 100.0;
    }
    ui.set_item_tooltip("Data opacity (%)");
}

pub(crate) fn draw_pixel_style_controls(
    ui: &Ui,
    id: &str,
    pixel_style: &mut usize,
    has_bounds: bool,
    palette: &Palette,
    width: f32,
) {
    if !has_bounds {
        *pixel_style = 0;
    }
    for style in 0..3 {
        if pixel_style_button(
            ui,
            id,
            style,
            *pixel_style == style,
            style == 0 || has_bounds,
            palette,
            width,
        ) {
            *pixel_style = style;
        }
    }
}

/// Match hover geometry to the visible pixel style instead of its center alone.
pub(crate) fn pixel_hover_distance(
    mouse: [f32; 2],
    center: [f32; 2],
    footprint: Option<&[[f32; 2]; 4]>,
    style: usize,
) -> Option<f32> {
    let squared_distance = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let Some(corners) = footprint.filter(|_| style != 0) else {
        let distance = squared_distance(mouse, center);
        return (distance <= 100.0).then_some(distance);
    };
    let edge_distance = (0..4)
        .map(|index| {
            let a = corners[index];
            let b = corners[(index + 1) % 4];
            let edge = [b[0] - a[0], b[1] - a[1]];
            let length_squared = edge[0].powi(2) + edge[1].powi(2);
            let t = if length_squared > 0.0 {
                ((mouse[0] - a[0]) * edge[0] + (mouse[1] - a[1]) * edge[1]) / length_squared
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            squared_distance(mouse, [a[0] + t * edge[0], a[1] + t * edge[1]])
        })
        .fold(f32::INFINITY, f32::min);
    if style == 1 {
        return (edge_distance <= 25.0).then_some(edge_distance);
    }
    let mut inside = false;
    for index in 0..4 {
        let a = corners[index];
        let b = corners[(index + 1) % 4];
        if (a[1] > mouse[1]) != (b[1] > mouse[1])
            && mouse[0] < (b[0] - a[0]) * (mouse[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    (inside || edge_distance <= 2.25).then_some(0.0)
}

pub(crate) fn draw_geo_view_controls(
    ui: &Ui,
    id: &str,
    options: &mut GeoViewOptions,
    shape: Option<&[usize]>,
    pinned_axis_name: &str,
    palette: &Palette,
) {
    draw_layer_controls(
        ui,
        id,
        &mut options.dummy_color,
        &mut options.dummy_rgb,
        &mut options.opacity,
        palette,
    );
    if let Some(shape) = shape.filter(|shape| shape.len() >= 3) {
        let mut time_index = options.time.map_or(0, |index| index + 1) as i32;
        if ui
            .slider_config("Time index (0 = all)", 0, shape[0] as i32)
            .build(&mut time_index)
        {
            options.time = (time_index > 0).then_some((time_index - 1) as usize);
        }
    }
    if let Some(shape) = shape.filter(|shape| shape.len() == 4) {
        let mut pinned = options.pinned.min(shape[3] - 1) as i32;
        if ui
            .slider_config(
                format!("Pinned {pinned_axis_name} index"),
                0,
                shape[3] as i32 - 1,
            )
            .build(&mut pinned)
        {
            options.pinned = pinned as usize;
        }
    }
}

pub(crate) struct GeoControlChanges {
    pub(crate) site_changed: bool,
    pub(crate) refresh_data: bool,
}

pub(crate) fn draw_geo_controls(
    ui: &Ui,
    id: &str,
    controls: &mut GeoViewControls,
    shape: Option<&[usize]>,
    pinned_axis_name: &str,
    has_bounds: bool,
    has_scan: bool,
    selected_pics: &mut usize,
    selected_site_group: &mut Option<String>,
    settings: &Settings,
) -> GeoControlChanges {
    let mut changes = GeoControlChanges {
        site_changed: false,
        refresh_data: false,
    };
    let action_width = geo_action_width(ui, settings);
    let special_width = geo_special_width(has_scan);
    if let Some(_table) = begin_control_columns_with_special(
        ui,
        &format!("{id}_controls"),
        special_width,
        action_width,
    ) {
        draw_geo_view_controls(
            ui,
            &format!("##{id}_pixel_style"),
            &mut controls.draft,
            shape,
            pinned_axis_name,
            &settings.palette,
        );
        changes.site_changed = draw_pics_controls(
            ui,
            id,
            &mut controls.draft.show_pics,
            selected_pics,
            selected_site_group,
            &settings.pics_sites,
        );
        ui.table_next_column();
        let available = ui.content_region_avail()[0].max(1.0);
        if has_scan {
            let button_width = ((available - PANEL_GAP) * 0.5)
                .min(GEO_SPECIAL_BUTTON_WIDTH)
                .max(1.0);
            let special_left = ui.cursor_screen_pos()[0];
            ui.group(|| {
                draw_pixel_style_controls(
                    ui,
                    id,
                    &mut controls.draft.pixel_style,
                    has_bounds,
                    &settings.palette,
                    button_width,
                );
            });
            let pixel_right = ui.item_rect_max()[0];
            ui.same_line_with_spacing(0.0, PANEL_GAP);
            ui.group(|| {
                draw_scan_filter_controls(
                    ui,
                    id,
                    &mut controls.draft.scan_filter,
                    &settings.palette,
                    button_width,
                );
            });
            debug_assert!(ui.item_rect_min()[0] >= pixel_right + PANEL_GAP - 1.0);
            debug_assert!(ui.item_rect_max()[0] <= special_left + available + 1.0);
        } else {
            draw_pixel_style_controls(
                ui,
                id,
                &mut controls.draft.pixel_style,
                has_bounds,
                &settings.palette,
                available.min(GEO_SPECIAL_BUTTON_WIDTH),
            );
        }
        controls.applied.pixel_style = controls.draft.pixel_style;
        if has_scan {
            controls.applied.scan_filter = controls.draft.scan_filter;
        }
        ui.table_next_column();
        let pending = controls.draft != controls.applied;
        if draw_update_button_with_height(ui, settings, pending, geo_update_height(ui, settings)) {
            controls.applied = controls.draft;
            changes.refresh_data = true;
        }
    }
    changes
}

pub(crate) fn draw_pics_controls(
    ui: &Ui,
    id: &str,
    show_pics: &mut bool,
    selected_pics: &mut usize,
    selected_group: &mut Option<String>,
    sites: &[PicsSite],
) -> bool {
    let available = ui.content_region_avail()[0];
    ui.checkbox(format!("Cal/val sites##{id}"), show_pics);
    if !*show_pics {
        return false;
    }
    let mut groups: Vec<&str> = sites.iter().map(|site| site.category.as_str()).collect();
    groups.sort_unstable();
    groups.dedup();
    let mut group_labels = vec!["All groups"];
    group_labels.extend(groups.iter().copied());
    let mut group_index = selected_group
        .as_deref()
        .and_then(|selected| groups.iter().position(|group| *group == selected))
        .map_or(0, |index| index + 1);
    if available >= 480.0 {
        ui.same_line();
    }
    ui.set_next_item_width(150.0_f32.min(available));
    if ui.combo_simple_string(
        format!("##site_group_{id}"),
        &mut group_index,
        &group_labels,
    ) {
        *selected_group = group_index
            .checked_sub(1)
            .map(|index| groups[index].to_owned());
        *selected_pics = 0;
    }
    let site_indices: Vec<usize> = sites
        .iter()
        .enumerate()
        .filter(|(_, site)| {
            selected_group
                .as_deref()
                .is_none_or(|group| site.category == group)
        })
        .map(|(index, _)| index)
        .collect();
    let mut site_labels = vec!["All sites"];
    site_labels.extend(site_indices.iter().map(|&index| sites[index].name.as_str()));
    let mut local_index = selected_pics
        .checked_sub(1)
        .and_then(|selected| site_indices.iter().position(|&index| index == selected))
        .map_or(0, |index| index + 1);
    if available >= 350.0 {
        ui.same_line();
    }
    ui.set_next_item_width(160.0_f32.min(available));
    let changed =
        ui.combo_simple_string(format!("##pics_site_{id}"), &mut local_index, &site_labels);
    if changed {
        *selected_pics = local_index
            .checked_sub(1)
            .map_or(0, |index| site_indices[index] + 1);
    }
    ui.set_item_tooltip("Select a site to center the view on its region.");
    changed
}

/// Apply the same continuous color scale and special-value colors in both geospatial views.
pub(crate) fn observation_color(
    point: &MapPoint,
    range: Option<(f64, f64)>,
    dummy_color: bool,
    dummy_rgb: [f32; 3],
    plot_ctx: &PlotContext,
    settings: &Settings,
    palette: &Palette,
) -> [f32; 4] {
    point.special.map_or_else(
        || {
            if dummy_color {
                return [dummy_rgb[0], dummy_rgb[1], dummy_rgb[2], 1.0];
            }
            let fraction = range.map_or(0.5, |(lo, hi)| {
                if hi > lo {
                    ((point.value - lo) / (hi - lo)) as f32
                } else {
                    0.5
                }
            });
            continuous_color(
                plot_ctx,
                fraction,
                settings.heatmap_colormap_preset,
                palette,
            )
        },
        |special| special.color(palette),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_tooltip_follows_the_selected_geometry() {
        let corners = [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]];
        let center = [10.0, 10.0];
        assert!(pixel_hover_distance(center, center, Some(&corners), 0).is_some());
        assert!(pixel_hover_distance(center, center, Some(&corners), 1).is_none());
        assert!(pixel_hover_distance([10.0, 2.0], center, Some(&corners), 1).is_some());
        assert!(pixel_hover_distance(center, center, Some(&corners), 2).is_some());
        assert!(pixel_hover_distance([30.0, 10.0], center, Some(&corners), 2).is_none());
        assert!(pixel_hover_distance(center, center, None, 2).is_some());
    }

    #[test]
    fn geospatial_icon_controls_render_in_one_frame() {
        use dear_app::imgui::{Condition, Context};
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([900.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let settings = Settings::load().unwrap();
        let mut map = GeoViewControls::new(true);
        let mut globe = GeoViewControls::new(true);
        let mut selected_pics = 0;
        let mut selected_group = None;
        let ui = context.frame();
        ui.window("Geo controls smoke")
            .size([900.0, 600.0], Condition::Always)
            .build(|| {
                draw_geo_controls(
                    ui,
                    "map",
                    &mut map,
                    Some(&[1, 8, 4, 16]),
                    "spectral",
                    true,
                    true,
                    &mut selected_pics,
                    &mut selected_group,
                    &settings,
                );
                draw_geo_controls(
                    ui,
                    "globe",
                    &mut globe,
                    Some(&[1, 8, 4, 16]),
                    "spectral",
                    true,
                    false,
                    &mut selected_pics,
                    &mut selected_group,
                    &settings,
                );
            });
        ui.window("Narrow geo controls smoke")
            .size([380.0, 360.0], Condition::Always)
            .build(|| {
                draw_geo_controls(
                    ui,
                    "narrow_map",
                    &mut map,
                    Some(&[1, 8, 4, 16]),
                    "spectral",
                    true,
                    true,
                    &mut selected_pics,
                    &mut selected_group,
                    &settings,
                );
            });
    }
}
