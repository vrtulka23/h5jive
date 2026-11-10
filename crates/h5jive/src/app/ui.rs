use super::*;

pub(super) fn control_action_width(ui: &Ui, settings: &Settings) -> f32 {
    settings
        .update_button_width
        .min(ui.content_region_avail()[0] * 0.3)
}

pub(super) fn begin_control_columns<'ui>(
    ui: &'ui Ui,
    id: &str,
    action_width: f32,
) -> Option<dear_app::imgui::TableToken<'ui>> {
    let width = ui.content_region_avail()[0];
    let table = ui.begin_table(id, 2)?;
    ui.table_setup_column_fixed_width(
        "Controls",
        TableColumnFlags::NONE,
        (width - action_width - PANEL_GAP).max(1.0),
    );
    ui.table_setup_column_fixed_width("Actions", TableColumnFlags::NONE, action_width.max(1.0));
    ui.table_next_row();
    ui.table_next_column();
    Some(table)
}

pub(super) fn begin_control_columns_with_special<'ui>(
    ui: &'ui Ui,
    id: &str,
    special_width: f32,
    action_width: f32,
) -> Option<dear_app::imgui::TableToken<'ui>> {
    let width = ui.content_region_avail()[0];
    let table = ui.begin_table(id, 3)?;
    ui.table_setup_column_fixed_width(
        "Controls",
        TableColumnFlags::NONE,
        (width - special_width - action_width - 2.0 * PANEL_GAP).max(1.0),
    );
    ui.table_setup_column_fixed_width("Special", TableColumnFlags::NONE, special_width.max(1.0));
    ui.table_setup_column_fixed_width("Update", TableColumnFlags::NONE, action_width.max(1.0));
    ui.table_next_row();
    ui.table_next_column();
    Some(table)
}

pub(super) fn draw_control_surface(
    draw: &DrawListMut<'_>,
    min: [f32; 2],
    max: [f32; 2],
    background: [f32; 4],
    border: [f32; 4],
    rounding: f32,
) {
    draw.add_rect(min, max, background)
        .rounding(rounding)
        .filled(true)
        .build();
    draw.add_rect(min, max, border).rounding(rounding).build();
}

pub(super) fn draw_plot_navigation(
    ui: &Ui,
    id: &str,
    origin: [f32; 2],
    width: f32,
    zoom: f32,
) -> PlotNavAction {
    ui.set_cursor_screen_pos(origin);
    let reset = ui.button_with_size(format!("Reset##plot_nav_{id}"), [width.max(1.0), 22.0]);
    ui.set_item_tooltip("Restore the initial view");
    ui.set_cursor_screen_pos([origin[0], origin[1] + 22.0 + CONTROL_INLINE_GAP]);
    let label = format!("{zoom:.1}×");
    let button_width =
        ((width - ui.calc_text_size(&label)[0] - 2.0 * CONTROL_INLINE_GAP) * 0.5).max(1.0);
    let out = ui.button_with_size(format!("<##plot_nav_out_{id}"), [button_width, 0.0]);
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    ui.text(label);
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    let inside = ui.button_with_size(format!(">##plot_nav_in_{id}"), [button_width, 0.0]);
    if reset {
        PlotNavAction::Reset
    } else if out {
        PlotNavAction::ZoomOut
    } else if inside {
        PlotNavAction::ZoomIn
    } else {
        PlotNavAction::None
    }
}

pub(super) fn draw_visibility_navigation(
    ui: &Ui,
    id: &str,
    origin: [f32; 2],
    width: f32,
    zoom: f32,
    visibility: &mut ValueVisibility,
    palette: &Palette,
) -> (PlotNavAction, bool) {
    ui.set_cursor_screen_pos(origin);
    let changed = visibility.draw_panel(ui, palette, width);
    let nav = draw_plot_navigation(
        ui,
        id,
        [origin[0], origin[1] + VALUE_GRID_HEIGHT + PANEL_GAP],
        width,
        zoom,
    );
    (nav, changed)
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use dear_app::imgui::{Condition, Context};

    #[test]
    fn overlay_navigation_does_not_scroll_a_child_panel() {
        let mut context = Context::create();
        let settings = Settings::load().unwrap();
        let mut visibility = ValueVisibility::default();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([700.0, 500.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        for frame in 0..2 {
            let ui = context.frame();
            ui.window("Volume layout smoke")
                .size([600.0, 400.0], Condition::Always)
                .build(|| {
                    ui.child_window("volume_panel")
                        .size([0.0, 280.0])
                        .build(ui, || {
                            let available = ui.content_region_avail();
                            let origin = ui.cursor_screen_pos();
                            ui.invisible_button(
                                "##volume_orbit",
                                [available[0] - 130.0, available[1] - 2.0],
                            );
                            draw_visibility_navigation(
                                ui,
                                "volume",
                                [origin[0] + available[0] - 126.0, origin[1]],
                                126.0,
                                1.0,
                                &mut visibility,
                                &settings.palette,
                            );
                            if frame == 1 {
                                assert_eq!(ui.scroll_max_y(), 0.0);
                            }
                        });
                });
            context.end_frame();
        }
    }
}

pub(super) fn draw_update_button(ui: &Ui, settings: &Settings, pending: bool) -> bool {
    draw_update_button_with_height(ui, settings, pending, settings.update_button_height)
}

pub(super) fn draw_update_button_with_height(
    ui: &Ui,
    settings: &Settings,
    pending: bool,
    height: f32,
) -> bool {
    let clicked = ui.button_with_size("Update", [ui.content_region_avail()[0].max(1.0), height]);
    if pending {
        let draw = ui.get_window_draw_list();
        draw.add_rect(
            ui.item_rect_min(),
            ui.item_rect_max(),
            settings.palette.accent,
        )
        .rounding(ui.clone_style().frame_rounding())
        .thickness(3.0)
        .build();
        ui.set_item_tooltip("Changes pending - click Update");
    }
    clicked
}

pub(super) fn draw_workspace_mode_icon(
    draw: &DrawListMut<'_>,
    mode: WorkspaceMode,
    origin: [f32; 2],
    color: [f32; 4],
) {
    let p = |x: f32, y: f32| [origin[0] + x, origin[1] + y];
    match mode {
        WorkspaceMode::DatasetBrowser => {
            draw.add_rect(p(1.5, 5.0), p(18.5, 17.0), color)
                .rounding(2.0)
                .thickness(1.5)
                .build();
            draw.add_line(p(2.0, 5.0), p(7.0, 5.0), color)
                .thickness(1.5)
                .build();
            draw.add_line(p(3.0, 3.0), p(8.0, 3.0), color)
                .thickness(1.5)
                .build();
            draw.add_line(p(8.0, 3.0), p(10.0, 5.0), color)
                .thickness(1.5)
                .build();
            draw.add_line(p(5.0, 10.0), p(15.0, 10.0), color).build();
            draw.add_line(p(5.0, 13.0), p(12.0, 13.0), color).build();
        }
        WorkspaceMode::DataInspector => {
            draw.add_rect(p(2.0, 2.0), p(13.0, 16.0), color)
                .rounding(1.5)
                .thickness(1.5)
                .build();
            for y in [5.0, 8.0, 11.0] {
                draw.add_line(p(5.0, y), p(10.0, y), color).build();
            }
            draw.add_circle(p(13.0, 12.0), 4.0, color)
                .thickness(1.5)
                .build();
            draw.add_line(p(15.8, 14.8), p(19.0, 18.0), color)
                .thickness(1.5)
                .build();
        }
        WorkspaceMode::Comparison => {
            draw.add_rect(p(1.5, 3.0), p(9.0, 16.0), color)
                .rounding(1.0)
                .thickness(1.5)
                .build();
            draw.add_rect(p(11.0, 3.0), p(18.5, 16.0), color)
                .rounding(1.0)
                .thickness(1.5)
                .build();
            draw.add_line(p(4.0, 7.0), p(7.0, 7.0), color).build();
            draw.add_line(p(13.5, 7.0), p(16.5, 7.0), color).build();
            draw.add_line(p(4.0, 11.0), p(7.0, 11.0), color).build();
            draw.add_line(p(13.5, 11.0), p(16.5, 11.0), color).build();
        }
    }
}

pub(super) fn draw_loading_indicator(
    ui: &Ui,
    top_right: [f32; 2],
    count: usize,
    palette: &Palette,
) {
    let draw = ui.get_foreground_draw_list();
    let label = if count == 1 {
        "Loading".to_owned()
    } else {
        format!("Loading {count}")
    };
    let text_size = ui.calc_text_size(&label);
    let height = 25.0;
    let width = text_size[0] + 42.0;
    let min = [top_right[0] - width, top_right[1] - height * 0.5];
    let max = [top_right[0], top_right[1] + height * 0.5];
    let center = [min[0] + 16.0, top_right[1]];
    let rounding = ui.clone_style().frame_rounding();
    draw.add_rect(min, max, palette.panel)
        .rounding(rounding)
        .filled(true)
        .build();
    draw.add_rect(min, max, palette.accent)
        .rounding(rounding)
        .thickness(1.5)
        .build();
    draw.add_text(
        [min[0] + 32.0, top_right[1] - text_size[1] * 0.5],
        palette.text,
        &label,
    );
    let rotation = ui.time() as f32 * 4.0;
    for index in 0..10 {
        let angle = rotation + index as f32 * std::f32::consts::TAU / 10.0;
        let direction = [angle.cos(), angle.sin()];
        let alpha = 0.25 + 0.75 * (index as f32 + 1.0) / 10.0;
        let color = [
            palette.accent[0],
            palette.accent[1],
            palette.accent[2],
            alpha,
        ];
        draw.add_line(
            [
                center[0] + direction[0] * 5.0,
                center[1] + direction[1] * 5.0,
            ],
            [
                center[0] + direction[0] * 9.0,
                center[1] + direction[1] * 9.0,
            ],
            color,
        )
        .thickness(2.0)
        .build();
    }
}

pub(crate) fn draw_view_icon(
    ui: &Ui,
    mode: ViewMode,
    min: [f32; 2],
    max: [f32; 2],
    selected: bool,
    hovered: bool,
    palette: &Palette,
) {
    let draw = ui.get_window_draw_list();
    let background = if selected {
        palette.view_selected
    } else if hovered {
        palette.view_hover
    } else {
        palette.view_idle
    };
    let stroke = palette.view_stroke;
    let border = if selected {
        palette.view_border_selected
    } else {
        palette.view_border_idle
    };
    draw_control_surface(
        &draw,
        min,
        max,
        background,
        border,
        ui.clone_style().frame_rounding(),
    );
    let width = max[0] - min[0];
    let height = max[1] - min[1];
    let side = width.min(height).max(0.0);
    let icon_min = [
        min[0] + (width - side) * 0.5,
        min[1] + (height - side) * 0.5,
    ];
    let p = |x: f32, y: f32| [icon_min[0] + side * x, icon_min[1] + side * y];
    match mode {
        ViewMode::Spatial3d => {
            draw.add_rect(p(0.22, 0.38), p(0.64, 0.78), stroke).build();
            draw.add_rect(p(0.37, 0.23), p(0.78, 0.63), stroke).build();
            for (a, b) in [
                ((0.22, 0.38), (0.37, 0.23)),
                ((0.64, 0.38), (0.78, 0.23)),
                ((0.22, 0.78), (0.37, 0.63)),
                ((0.64, 0.78), (0.78, 0.63)),
            ] {
                draw.add_line(p(a.0, a.1), p(b.0, b.1), stroke).build();
            }
        }
        ViewMode::Slice2d => {
            draw.add_rect(p(0.22, 0.23), p(0.78, 0.77), stroke).build();
            draw.add_rect(p(0.41, 0.41), p(0.59, 0.59), stroke)
                .filled(true)
                .build();
            for fraction in [0.41, 0.59] {
                draw.add_line(p(fraction, 0.23), p(fraction, 0.77), stroke)
                    .build();
                draw.add_line(p(0.22, fraction), p(0.78, fraction), stroke)
                    .build();
            }
        }
        ViewMode::Profiles1d => {
            draw.add_line(p(0.22, 0.76), p(0.78, 0.76), stroke).build();
            draw.add_line(p(0.22, 0.76), p(0.22, 0.24), stroke).build();
            for (a, b) in [
                ((0.27, 0.65), (0.40, 0.47)),
                ((0.40, 0.47), (0.53, 0.58)),
                ((0.53, 0.58), (0.74, 0.29)),
            ] {
                draw.add_line(p(a.0, a.1), p(b.0, b.1), stroke)
                    .thickness(1.8)
                    .build();
            }
        }
        ViewMode::WorldMap => {
            draw.add_rect(p(0.16, 0.30), p(0.84, 0.70), stroke).build();
            draw.add_line(p(0.16, 0.50), p(0.84, 0.50), stroke).build();
            for fraction in [0.36, 0.64] {
                draw.add_line(p(fraction, 0.30), p(fraction, 0.70), stroke)
                    .build();
            }
        }
        ViewMode::Globe => {
            draw.add_circle(p(0.50, 0.50), side * 0.28, stroke).build();
            draw.add_line(p(0.23, 0.50), p(0.77, 0.50), stroke).build();
            draw.add_line(p(0.50, 0.22), p(0.50, 0.78), stroke).build();
            draw.add_circle(p(0.50, 0.50), side * 0.14, stroke).build();
        }
        ViewMode::Data => {
            for y in [0.32, 0.50, 0.68] {
                draw.add_rect(p(0.22, y - 0.04), p(0.30, y + 0.04), stroke)
                    .filled(true)
                    .build();
                draw.add_line(p(0.39, y), p(0.78, y), stroke)
                    .thickness(1.7)
                    .build();
            }
        }
    }
}
pub(crate) fn find_entry<'a>(root: &'a Entry, path: &str) -> Option<&'a Entry> {
    if root.path == path {
        return Some(root);
    }
    root.children
        .iter()
        .find_map(|entry| find_entry(entry, path))
}
pub(crate) fn draw_entry(
    ui: &Ui,
    entry: &Entry,
    source: &File,
    file: usize,
    selected: &mut Option<Selection>,
    geolocation_cache: &mut HashMap<(usize, String), Option<Geolocation>>,
    dataset_rules: &[GeolocationRule],
    show_shape: bool,
    palette: &Palette,
) {
    let geolocation = match &entry.kind {
        Kind::Dataset {
            shape,
            numeric: true,
        } if matches!(shape.len(), 2..=4) => geolocation_cache
            .entry((file, entry.path.clone()))
            .or_insert_with(|| {
                source
                    .geolocation_with_rules(&entry.path, dataset_rules)
                    .ok()
                    .flatten()
            })
            .clone(),
        _ => None,
    };
    let visible_label = match &entry.kind {
        Kind::Group => format!("      {}", entry.name),
        Kind::Dataset { shape, .. } if show_shape => {
            format!("      {}  {:?}", entry.name, shape)
        }
        _ => format!("      {}", entry.name),
    };
    let label = format!("{visible_label}##{}", entry.path);
    if matches!(entry.kind, Kind::Group) {
        let is_selected = selected
            .as_ref()
            .is_some_and(|s| s.file == file && s.path == entry.path);
        let mut node = ui
            .tree_node_config(&label)
            .selected(is_selected)
            .open_on_arrow(true)
            .open_on_double_click(true)
            .span_avail_width(true);
        if selected.as_ref().is_some_and(|s| {
            s.file == file
                && s.path.starts_with(&entry.path)
                && s.path.as_bytes().get(entry.path.len()) == Some(&b'/')
        }) {
            node = node.opened(true, Condition::Once);
        }
        let node = node.push();
        draw_tree_entry_icons(ui, true, false, palette);
        if ui.is_item_clicked() {
            *selected = Some(Selection {
                file,
                path: entry.path.clone(),
            });
        }
        if let Some(_node) = node {
            for child in &entry.children {
                draw_entry(
                    ui,
                    child,
                    source,
                    file,
                    selected,
                    geolocation_cache,
                    dataset_rules,
                    show_shape,
                    palette,
                );
            }
        }
    } else {
        let is_selected = selected
            .as_ref()
            .is_some_and(|s| s.file == file && s.path == entry.path);
        if ui.selectable_config(&label).selected(is_selected).build() {
            *selected = Some(Selection {
                file,
                path: entry.path.clone(),
            });
        }
        draw_tree_entry_icons(ui, false, geolocation.is_some(), palette);
        if let Some(geolocation) = geolocation {
            ui.set_item_tooltip(format!(
                "Geolocated dataset\nLatitude: {}\nLongitude: {}",
                geolocation.latitude, geolocation.longitude
            ));
        }
    }
}

fn draw_tree_entry_icons(ui: &Ui, group: bool, geolocated: bool, palette: &Palette) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let x = min[0]
        + if group {
            ui.text_line_height() + 4.0
        } else {
            4.0
        };
    let y = (min[1] + max[1]) * 0.5;
    let color = if group { palette.accent } else { palette.muted };
    let draw = ui.get_window_draw_list();
    if group {
        // A tab and body make the folder readable even at small font sizes.
        draw.add_rect([x, y - 6.0], [x + 6.0, y - 3.0], color)
            .filled(true)
            .build();
        draw.add_rect([x, y - 3.0], [x + 13.0, y + 6.0], color)
            .rounding(1.0)
            .filled(true)
            .build();
    } else {
        draw.add_rect([x, y - 6.0], [x + 12.0, y + 6.0], color)
            .rounding(1.0)
            .build();
        for offset in [-2.0, 2.0] {
            draw.add_line([x + 3.0, y + offset], [x + 9.0, y + offset], color)
                .build();
        }
    }
    if geolocated {
        let center = [x + 18.0, y];
        let color = palette.accent;
        draw.add_circle(center, 5.0, color).thickness(1.2).build();
        draw.add_line(
            [center[0] - 4.5, center[1]],
            [center[0] + 4.5, center[1]],
            color,
        )
        .build();
        draw.add_line(
            [center[0], center[1] - 4.5],
            [center[0], center[1] + 4.5],
            color,
        )
        .build();
        draw.add_circle(center, 2.7, color).thickness(1.0).build();
    }
}

pub(super) fn detail_row(ui: &Ui, name: &str, value: impl AsRef<str>) {
    ui.table_next_row();
    ui.table_next_column();
    ui.text(name);
    ui.table_next_column();
    ui.text_wrapped(value);
}
pub(super) fn detail_row_special(
    ui: &Ui,
    name: &str,
    special: SpecialValue,
    count: u128,
    palette: &Palette,
) {
    detail_row_tinted(ui, name, &count.to_string(), special.color(palette));
}
pub(super) fn detail_row_tinted(ui: &Ui, name: &str, value: &str, color: [f32; 4]) {
    ui.table_next_row();
    ui.table_next_column();
    ui.text_colored(color, name);
    ui.table_next_column();
    ui.text_colored(color, value);
}
pub(super) fn draw_splitter(
    ui: &Ui,
    id: &str,
    size: [f32; 2],
    vertical: bool,
    palette: &Palette,
) -> f32 {
    ui.invisible_button(id, size);
    let active = ui.is_item_active();
    let highlighted = active || ui.is_item_hovered();
    if highlighted {
        ui.set_mouse_cursor(Some(if vertical {
            MouseCursor::ResizeEW
        } else {
            MouseCursor::ResizeNS
        }));
    }
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let color = if highlighted {
        palette.splitter_active
    } else {
        palette.splitter_idle
    };
    let draw = ui.get_window_draw_list();
    if vertical {
        let x = (min[0] + max[0]) * 0.5;
        draw.add_line([x, min[1]], [x, max[1]], color)
            .thickness(2.0)
            .build();
    } else {
        let y = (min[1] + max[1]) * 0.5;
        draw.add_line([min[0], y], [max[0], y], color)
            .thickness(2.0)
            .build();
    }
    if active {
        ui.io().mouse_delta()[if vertical { 0 } else { 1 }]
    } else {
        0.0
    }
}
pub(super) fn table_extent(
    ui: &Ui,
    content_width: f32,
    rows: usize,
    padding: f32,
) -> ([f32; 2], bool) {
    let available = ui.content_region_avail();
    let vertical_padding = ui.clone_style().cell_padding()[1];
    let content_height = ui.frame_height()
        + rows as f32 * (ui.text_line_height() + 2.0 * vertical_padding)
        + 2.0 * vertical_padding;
    let scroll = content_width + 8.0 > available[0] || content_height + 8.0 > available[1];
    if scroll {
        let scrollbar_space = padding;
        (
            [
                (content_width + scrollbar_space).min(available[0]).max(1.0),
                (content_height + scrollbar_space)
                    .min(available[1])
                    .max(1.0),
            ],
            true,
        )
    } else {
        ([0.0, 0.0], false)
    }
}
