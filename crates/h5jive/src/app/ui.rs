use super::*;

/// Draw a sampled or native heatmap with the same ImPlot settings in core views and plugins.
pub(super) fn plot_heatmap_grid(
    plot_ui: &PlotUi<'_>,
    label: &str,
    values: &[f64],
    grid: [usize; 2],
    range: (f64, f64),
    bounds: [f64; 4],
) {
    HeatmapPlot::new(label, values, grid[0], grid[1])
        .with_item_flags(ItemFlags::NO_LEGEND)
        .with_scale(range.0, range.1)
        .with_bounds(bounds[0], bounds[1], bounds[2], bounds[3])
        .without_value_labels()
        .plot(plot_ui);
}

/// Cover a filtered heatmap cell through its outer pixels so the underlying
/// ImPlot rectangle cannot leave a colored seam at fractional pixel edges.
pub(super) fn cover_heatmap_cell(
    draw: &DrawListMut<'_>,
    first: [f32; 2],
    last: [f32; 2],
    color: [f32; 4],
) {
    draw.add_rect(
        [first[0].floor(), first[1].floor()],
        [last[0].ceil(), last[1].ceil()],
        color,
    )
    .filled(true)
    .build();
}

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

/// A passive status badge anchored inside the figure, clear of its right rail.
pub(super) fn draw_render_badge(
    ui: &Ui,
    draw: &DrawListMut<'_>,
    min: [f32; 2],
    max: [f32; 2],
    status: &str,
    detail: &str,
    palette: &Palette,
) {
    let status_size = ui.calc_text_size(status);
    let detail_size = ui.calc_text_size(detail);
    let width = status_size[0].max(detail_size[0]) + 22.0;
    let height = status_size[1] + detail_size[1] + 19.0;
    if max[0] - min[0] < width + 24.0 || max[1] - min[1] < height + 24.0 {
        return;
    }
    let left = max[0] - width - 12.0;
    let top = min[1] + 12.0;
    let mut background = palette.panel;
    background[3] = 0.90;
    draw_control_surface(
        draw,
        [left, top],
        [left + width, top + height],
        background,
        palette.border,
        4.0,
    );
    draw.add_text([left + 11.0, top + 7.0], palette.accent, status);
    draw.add_text(
        [left + 11.0, top + 10.0 + status_size[1]],
        palette.text,
        detail,
    );
}

pub(super) fn geo_render_badge(data: &MapData, current: bool) -> (&'static str, String) {
    let status = if !current {
        "Updating"
    } else if data.preview {
        "Preview"
    } else if data.aggregated {
        "Aggregated"
    } else if data.sampled {
        "Sampled"
    } else {
        "Native"
    };
    let unit = if data.aggregated { "cells" } else { "points" };
    (status, format!("{} {unit}", data.points.len()))
}

pub(super) fn draw_plot_navigation(
    ui: &Ui,
    id: &str,
    origin: [f32; 2],
    width: f32,
    zoom: f32,
    tutorial_tips: bool,
    center_available: Option<bool>,
) -> PlotNavAction {
    ui.set_cursor_screen_pos(origin);
    let reset_width = if center_available.is_some() {
        ((width - CONTROL_INLINE_GAP) * 0.5).max(1.0)
    } else {
        width.max(1.0)
    };
    let reset = ui.button_with_size(format!("Reset##plot_nav_{id}"), [reset_width, 22.0]);
    item_tip(
        ui,
        tutorial_tips,
        "Restore the initial view",
        "Return to the initial zoom and position.",
    );
    let center = if let Some(available) = center_available {
        ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
        let _disabled = ui.begin_disabled_with_cond(!available);
        let clicked =
            ui.button_with_size(format!("Center##plot_nav_center_{id}"), [reset_width, 22.0]);
        item_tip(
            ui,
            tutorial_tips,
            "Rotate toward the selected dataset.",
            "Turn the globe toward the selected dataset.",
        );
        clicked
    } else {
        false
    };
    ui.set_cursor_screen_pos([origin[0], origin[1] + 22.0 + CONTROL_INLINE_GAP]);
    let label = format!("{zoom:.1}×");
    let button_width =
        ((width - ui.calc_text_size(&label)[0] - 2.0 * CONTROL_INLINE_GAP) * 0.5).max(1.0);
    let out = ui.button_with_size(format!("<##plot_nav_out_{id}"), [button_width, 0.0]);
    item_tip(
        ui,
        tutorial_tips,
        "",
        "Zoom out to see more of the dataset.",
    );
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    ui.text(label);
    ui.same_line_with_spacing(0.0, CONTROL_INLINE_GAP);
    let inside = ui.button_with_size(format!(">##plot_nav_in_{id}"), [button_width, 0.0]);
    item_tip(ui, tutorial_tips, "", "Zoom in for more detail.");
    if reset {
        PlotNavAction::Reset
    } else if center {
        PlotNavAction::Center
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
    tutorial_tips: bool,
    center_available: Option<bool>,
) -> (PlotNavAction, bool) {
    ui.set_cursor_screen_pos(origin);
    let changed = visibility.draw_panel(ui, palette, width, tutorial_tips);
    let nav = draw_plot_navigation(
        ui,
        id,
        [origin[0], origin[1] + VALUE_GRID_HEIGHT + PANEL_GAP],
        width,
        zoom,
        tutorial_tips,
        center_available,
    );
    (nav, changed)
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use dear_app::imgui::{Condition, Context, MouseButton};

    #[test]
    fn file_name_opens_picker_while_file_icon_selects_root() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arrays/space_demo.hdf5");
        let file = File::open(&path).unwrap();
        let palette = Settings::load().unwrap().palette;
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 300.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut selected = None;
        let draw = |context: &mut Context, selected: &mut Option<Selection>| {
            let ui = context.frame();
            let mut clicked_name = false;
            let mut name_x = 0.0;
            let mut icon_x = 0.0;
            let mut row_y = 0.0;
            ui.window("File tree click")
                .position([0.0, 0.0], Condition::Always)
                .size([400.0, 150.0], Condition::Always)
                .build(|| {
                    let row_x = ui.cursor_screen_pos()[0];
                    let prefix = tree_label_prefix(ui, row_x, false);
                    name_x = row_x + ui.calc_text_size(format!("{prefix}X"))[0]
                        - ui.calc_text_size("X")[0];
                    icon_x = tree_icon_x(ui, row_x, false, false);
                    clicked_name = draw_file_root(ui, &file, 0, selected, false, &palette);
                    row_y = (ui.item_rect_min()[1] + ui.item_rect_max()[1]) * 0.5;
                });
            context.end_frame();
            (clicked_name, name_x, icon_x, row_y)
        };
        let (_, name_x, icon_x, row_y) = draw(&mut context, &mut selected);
        context.io_mut().add_mouse_pos_event([name_x + 4.0, row_y]);
        draw(&mut context, &mut selected);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        let pressed = draw(&mut context, &mut selected).0;
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, false);
        let released = draw(&mut context, &mut selected).0;
        assert!(pressed || released);
        assert!(selected.is_none());
        context.io_mut().add_mouse_pos_event([icon_x + 5.0, row_y]);
        draw(&mut context, &mut selected);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        draw(&mut context, &mut selected);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, false);
        draw(&mut context, &mut selected);
        assert_eq!(
            selected.as_ref().map(|selection| selection.path.as_str()),
            Some("/")
        );
    }

    #[test]
    fn globe_center_button_reports_center_action() {
        use dear_app::imgui::MouseButton;

        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([300.0, 200.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let draw = |context: &mut Context| {
            let ui = context.frame();
            let mut action = PlotNavAction::None;
            ui.window("Globe navigation")
                .position([0.0, 0.0], Condition::Always)
                .size([200.0, 140.0], Condition::Always)
                .build(|| {
                    action = draw_plot_navigation(
                        ui,
                        "globe",
                        [20.0, 40.0],
                        110.0,
                        1.0,
                        false,
                        Some(true),
                    );
                });
            context.end_frame();
            action
        };
        assert_eq!(draw(&mut context), PlotNavAction::None);
        context.io_mut().add_mouse_pos_event([105.0, 50.0]);
        assert_eq!(draw(&mut context), PlotNavAction::None);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        assert_eq!(draw(&mut context), PlotNavAction::None);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, false);
        assert_eq!(draw(&mut context), PlotNavAction::Center);
        context.io_mut().add_mouse_pos_event([45.0, 50.0]);
        assert_eq!(draw(&mut context), PlotNavAction::None);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        assert_eq!(draw(&mut context), PlotNavAction::None);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, false);
        assert_eq!(draw(&mut context), PlotNavAction::Reset);
    }

    #[test]
    fn value_table_row_height_is_stable_while_editing() {
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 300.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut next_row_positions = Vec::new();
        let mut cell_bounds = Vec::new();
        for editing in [false, true] {
            let ui = context.frame();
            ui.window("Stable value rows")
                .size([400.0, 200.0], Condition::Always)
                .build(|| {
                    let _padding = ui.push_style_var(StyleVar::CellPadding(TABLE_CELL_PADDING));
                    if let Some(_table) = ui.begin_table("values", 1) {
                        let row_height = ui.frame_height() + 2.0 * TABLE_CELL_PADDING[1];
                        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);
                        ui.table_next_column();
                        if editing {
                            let mut value = "42".to_owned();
                            ui.set_next_item_width(120.0);
                            ui.input_text("##cell", &mut value).build();
                        } else {
                            let _align = ui.push_style_var(StyleVar::ButtonTextAlign([0.0, 0.5]));
                            ui.button_with_size("42##cell", [120.0, ui.frame_height()]);
                        }
                        cell_bounds.push((ui.item_rect_min(), ui.item_rect_max()));
                        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);
                        ui.table_next_column();
                        ui.text("next row");
                        next_row_positions.push(ui.item_rect_min()[1]);
                    }
                });
            context.end_frame();
        }
        assert_eq!(next_row_positions.len(), 2);
        assert!((next_row_positions[0] - next_row_positions[1]).abs() < 0.5);
        for axis in 0..2 {
            assert!((cell_bounds[0].0[axis] - cell_bounds[1].0[axis]).abs() < 0.5);
            assert!((cell_bounds[0].1[axis] - cell_bounds[1].1[axis]).abs() < 0.5);
        }
    }

    #[test]
    fn loading_indicator_stays_centered_as_label_width_changes() {
        let center = [640.0, 15.0];
        for width in [55.0, 180.0, 330.0] {
            let (min, max) = loading_indicator_bounds(center, width);
            assert_eq!((min[0] + max[0]) * 0.5, center[0]);
            assert_eq!((min[1] + max[1]) * 0.5, center[1]);
            assert_eq!(max[0] - min[0], width + 42.0);
        }
    }

    #[test]
    fn selected_group_and_dataset_render_without_scope_errors() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arrays/space_demo.hdf5");
        let file = File::open(&path).unwrap();
        let settings = Settings::load().unwrap();
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 400.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut geolocation_cache = HashMap::new();
        for selected_path in ["/observations", "/observations/swath_2d"] {
            let mut selected = Some(Selection {
                file: 0,
                path: selected_path.into(),
            });
            let ui = context.frame();
            ui.window("Selected tree row").build(|| {
                for entry in &file.tree.children {
                    draw_entry(
                        ui,
                        entry,
                        &file,
                        0,
                        false,
                        &mut selected,
                        &mut geolocation_cache,
                        &settings.dataset_rules,
                        true,
                        settings.tutorial_tips,
                        &settings.palette,
                    );
                }
            });
            context.end_frame();
        }
    }

    #[test]
    fn double_click_on_selected_dataset_requests_reload() {
        use dear_app::imgui::MouseButton;

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arrays/space_demo.hdf5");
        let file = File::open(&path).unwrap();
        let entry = find_entry(&file.tree, "/observations/swath_2d").unwrap();
        let settings = Settings::load().unwrap();
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 300.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut selection = Some(Selection {
            file: 0,
            path: entry.path.clone(),
        });
        let mut geolocation_cache = HashMap::new();
        fn draw(
            context: &mut Context,
            entry: &Entry,
            file: &File,
            selection: &mut Option<Selection>,
            geolocation_cache: &mut HashMap<(usize, String), Option<Geolocation>>,
            settings: &Settings,
        ) -> (bool, ([f32; 2], [f32; 2])) {
            let ui = context.frame();
            let mut reload = false;
            let mut bounds = ([0.0; 2], [0.0; 2]);
            ui.window("Double click dataset")
                .position([0.0, 0.0], Condition::Always)
                .size([400.0, 200.0], Condition::Always)
                .build(|| {
                    reload = draw_entry(
                        ui,
                        entry,
                        &file,
                        0,
                        false,
                        selection,
                        geolocation_cache,
                        &settings.dataset_rules,
                        true,
                        false,
                        &settings.palette,
                    );
                    bounds = (ui.item_rect_min(), ui.item_rect_max());
                });
            context.end_frame();
            (reload, bounds)
        }
        let (_, (min, max)) = draw(
            &mut context,
            entry,
            &file,
            &mut selection,
            &mut geolocation_cache,
            &settings,
        );
        let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
        context.io_mut().add_mouse_pos_event(center);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        assert!(
            !draw(
                &mut context,
                entry,
                &file,
                &mut selection,
                &mut geolocation_cache,
                &settings
            )
            .0
        );
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, false);
        assert!(
            !draw(
                &mut context,
                entry,
                &file,
                &mut selection,
                &mut geolocation_cache,
                &settings
            )
            .0
        );
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        assert!(
            draw(
                &mut context,
                entry,
                &file,
                &mut selection,
                &mut geolocation_cache,
                &settings
            )
            .0
        );
    }

    #[test]
    fn dataset_icon_sits_below_parent_folder_icon() {
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 300.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let ui = context.frame();
        ui.window("Tree alignment").build(|| {
            let folder_row_x = ui.cursor_screen_pos()[0];
            let folder_icon_x = tree_icon_x(ui, folder_row_x, true, false);
            let root_prefix = tree_label_prefix(ui, folder_row_x, false);
            let root_name_x = folder_row_x + ui.calc_text_size(format!("{root_prefix}X"))[0]
                - ui.calc_text_size("X")[0];
            let root_geo_icon_right = tree_icon_x(ui, folder_row_x, false, false) + 29.0;
            assert!(root_name_x >= root_geo_icon_right + 6.0);
            let node = ui
                .tree_node_config("folder")
                .opened(true, Condition::Always)
                .push();
            if let Some(_node) = node {
                let dataset_row_x = ui.cursor_screen_pos()[0];
                let prefix = tree_label_prefix(ui, dataset_row_x, true);
                let name_x = dataset_row_x + ui.calc_text_size(format!("{prefix}X"))[0]
                    - ui.calc_text_size("X")[0];
                let geo_icon_right = tree_icon_x(ui, dataset_row_x, false, true) + 29.0;
                assert!(name_x >= geo_icon_right + 6.0);
                assert!((name_x - root_name_x).abs() <= ui.calc_text_size(" ")[0].max(4.0));
                ui.selectable_config("dataset").build();
                let dataset_icon_x = tree_icon_x(ui, dataset_row_x, false, true);
                assert!((dataset_icon_x - folder_icon_x).abs() < 0.5);
            } else {
                panic!("folder should be open");
            }
        });
    }

    #[test]
    fn root_dataset_icon_aligns_with_root_group_icon() {
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        context.io_mut().set_display_size([500.0, 300.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let ui = context.frame();
        ui.window("Root tree alignment").build(|| {
            let group_row_x = ui.cursor_screen_pos()[0];
            let group_icon_x = tree_icon_x(ui, group_row_x, true, false);
            ui.tree_node_config("group")
                .opened(false, Condition::Always)
                .push();
            let dataset_row_x = ui.cursor_screen_pos()[0];
            let dataset_icon_x = tree_icon_x(ui, dataset_row_x, false, false);
            assert!((dataset_icon_x - group_icon_x).abs() < 0.5);
        });
    }

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
                                settings.tutorial_tips,
                                None,
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
        item_tip(
            ui,
            settings.tutorial_tips,
            "Changes pending - click Update",
            "Apply the view controls to refresh what is shown.",
        );
    } else {
        item_tip(
            ui,
            settings.tutorial_tips,
            "",
            "Change a view control, then click Update to redraw.",
        );
    }
    clicked
}

pub(super) fn draw_accent_button(ui: &Ui, palette: &Palette, label: &str, size: [f32; 2]) -> bool {
    let _button = ui.push_style_color(StyleColor::Button, palette.accent);
    let _hover = ui.push_style_color(
        StyleColor::ButtonHovered,
        mix_color(palette.accent, palette.text, 0.18),
    );
    let _active = ui.push_style_color(
        StyleColor::ButtonActive,
        mix_color(palette.accent, palette.panel, 0.2),
    );
    if size == [0.0, 0.0] {
        ui.button(label)
    } else {
        ui.button_with_size(label, size)
    }
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
        WorkspaceMode::PluginLab => {
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

fn loading_indicator_bounds(center: [f32; 2], text_width: f32) -> ([f32; 2], [f32; 2]) {
    let half_width = (text_width + 42.0) * 0.5;
    (
        [center[0] - half_width, center[1] - 12.5],
        [center[0] + half_width, center[1] + 12.5],
    )
}

pub(super) fn draw_loading_indicator(ui: &Ui, center: [f32; 2], label: &str, palette: &Palette) {
    let draw = ui.get_foreground_draw_list();
    let text_size = ui.calc_text_size(label);
    let (min, max) = loading_indicator_bounds(center, text_size[0]);
    let spinner_center = [min[0] + 16.0, center[1]];
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
        [min[0] + 32.0, center[1] - text_size[1] * 0.5],
        palette.text,
        label,
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
                spinner_center[0] + direction[0] * 5.0,
                spinner_center[1] + direction[1] * 5.0,
            ],
            [
                spinner_center[0] + direction[0] * 9.0,
                spinner_center[1] + direction[1] * 9.0,
            ],
            color,
        )
        .thickness(2.0)
        .build();
    }
}

pub(super) fn draw_error_indicator(ui: &Ui, top_right: [f32; 2], label: &str, palette: &Palette) {
    let draw = ui.get_foreground_draw_list();
    let text_size = ui.calc_text_size(label);
    let height = 25.0;
    let width = text_size[0] + 42.0;
    let min = [top_right[0] - width, top_right[1] - height * 0.5];
    let max = [top_right[0], top_right[1] + height * 0.5];
    let color = [0.95, 0.38, 0.33, 1.0];
    let rounding = ui.clone_style().frame_rounding();
    draw.add_rect(min, max, palette.panel)
        .rounding(rounding)
        .filled(true)
        .build();
    draw.add_rect(min, max, color)
        .rounding(rounding)
        .thickness(1.5)
        .build();
    draw.add_text(
        [min[0] + 14.0, top_right[1] - text_size[1] * 0.5],
        color,
        "!",
    );
    draw.add_text(
        [min[0] + 32.0, top_right[1] - text_size[1] * 0.5],
        palette.text,
        label,
    );
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
fn tree_label_prefix(ui: &Ui, row_x: f32, has_parent: bool) -> String {
    // Reserve the geo icon column for every dataset so neighboring names align.
    let icon_right = tree_icon_x(ui, row_x, false, has_parent) + 29.0;
    let space_width = (ui.calc_text_size(" X")[0] - ui.calc_text_size("X")[0]).max(1.0);
    " ".repeat(((icon_right - row_x + 7.0) / space_width).ceil().max(0.0) as usize)
}

pub(super) fn draw_file_root(
    ui: &Ui,
    source: &File,
    file: usize,
    selected: &mut Option<Selection>,
    tutorial_tips: bool,
    palette: &Palette,
) -> bool {
    let row_x = ui.cursor_screen_pos()[0];
    let is_selected = selected
        .as_ref()
        .is_some_and(|current| current.file == file && current.path == "/");
    let prefix = tree_label_prefix(ui, row_x, false);
    let label = format!("{prefix}{}##file_root_{file}", source.name);
    let clicked = ui.selectable_config(&label).selected(is_selected).build();
    let name_x = row_x + ui.calc_text_size(format!("{prefix}X"))[0] - ui.calc_text_size("X")[0];
    let name_right = name_x + ui.calc_text_size(&source.name)[0];
    let pointer_x = ui.io().mouse_pos()[0];
    let name_hovered = ui.is_item_hovered() && (name_x..=name_right).contains(&pointer_x);
    if clicked && !name_hovered {
        *selected = Some(Selection {
            file,
            path: "/".into(),
        });
    }
    if is_selected {
        draw_selected_tree_row(ui, palette);
    }
    draw_tree_entry_icons(ui, row_x, false, false, false, palette);
    if name_hovered {
        ui.set_mouse_cursor(Some(MouseCursor::Hand));
        let y = ui.item_rect_max()[1] - 2.0;
        ui.get_window_draw_list()
            .add_line([name_x, y], [name_right, y], palette.accent)
            .build();
    }
    item_tip(
        ui,
        tutorial_tips,
        if name_hovered { "Choose a file" } else { "" },
        if name_hovered {
            "Click the file name to open the picker for this tab."
        } else {
            "Click the file icon to inspect root attributes."
        },
    );
    clicked && name_hovered
}

pub(crate) fn draw_entry(
    ui: &Ui,
    entry: &Entry,
    source: &File,
    file: usize,
    has_parent: bool,
    selected: &mut Option<Selection>,
    geolocation_cache: &mut HashMap<(usize, String), Option<Geolocation>>,
    dataset_rules: &[GeolocationRule],
    show_shape: bool,
    tutorial_tips: bool,
    palette: &Palette,
) -> bool {
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
    let row_x = ui.cursor_screen_pos()[0];
    let dataset_prefix = tree_label_prefix(ui, row_x, has_parent);
    let visible_label = match &entry.kind {
        Kind::Group => format!("      {}", entry.name),
        Kind::Dataset { shape, .. } if show_shape => {
            format!("{dataset_prefix}{}  {:?}", entry.name, shape)
        }
        Kind::Dataset { .. } => format!("{dataset_prefix}{}", entry.name),
        Kind::Other => format!("      {}", entry.name),
    };
    let label = format!("{visible_label}##{}", entry.path);
    if matches!(entry.kind, Kind::Group) {
        let row_x = ui.cursor_screen_pos()[0];
        let is_selected = selected
            .as_ref()
            .is_some_and(|s| s.file == file && s.path == entry.path);
        let node = {
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
            node.push()
        };
        let clicked = ui.is_item_clicked();
        if is_selected {
            draw_selected_tree_row(ui, palette);
        }
        draw_tree_entry_icons(ui, row_x, true, false, has_parent, palette);
        item_tip(
            ui,
            tutorial_tips,
            "",
            "Click to inspect this group; use its arrow to expand children.",
        );
        if clicked {
            *selected = Some(Selection {
                file,
                path: entry.path.clone(),
            });
        }
        let mut reload_requested = false;
        if let Some(_node) = node {
            for child in &entry.children {
                reload_requested |= draw_entry(
                    ui,
                    child,
                    source,
                    file,
                    true,
                    selected,
                    geolocation_cache,
                    dataset_rules,
                    show_shape,
                    tutorial_tips,
                    palette,
                );
            }
        }
        reload_requested
    } else {
        let row_x = ui.cursor_screen_pos()[0];
        let is_selected = selected
            .as_ref()
            .is_some_and(|s| s.file == file && s.path == entry.path);
        let clicked = ui.selectable_config(&label).selected(is_selected).build();
        let is_dataset = matches!(entry.kind, Kind::Dataset { .. });
        let reload_requested = is_dataset
            && ui.is_item_hovered()
            && ui.is_mouse_double_clicked(dear_app::imgui::MouseButton::Left);
        let normal_tip = if is_dataset {
            geolocation
                .as_ref()
                .map(|geo| {
                    format!(
                        "Geolocated dataset\nLatitude: {}\nLongitude: {}\nDouble-click to reload.",
                        geo.latitude, geo.longitude
                    )
                })
                .unwrap_or_else(|| "Double-click to reload this dataset.".into())
        } else {
            String::new()
        };
        if clicked || reload_requested {
            *selected = Some(Selection {
                file,
                path: entry.path.clone(),
            });
        }
        if is_selected {
            draw_selected_tree_row(ui, palette);
        }
        draw_tree_entry_icons(ui, row_x, false, geolocation.is_some(), has_parent, palette);
        item_tip(
            ui,
            tutorial_tips,
            &normal_tip,
            if is_dataset {
                "Click to select this dataset; double-click to reload it."
            } else {
                "Select this entry to inspect it."
            },
        );
        reload_requested
    }
}

fn draw_selected_tree_row(ui: &Ui, palette: &Palette) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let draw = ui.get_window_draw_list();
    let fill = [
        palette.accent[0],
        palette.accent[1],
        palette.accent[2],
        0.24,
    ];
    draw.add_rect(min, max, fill)
        .rounding(3.0)
        .filled(true)
        .build();
    draw.add_rect(min, max, palette.accent)
        .rounding(3.0)
        .thickness(1.5)
        .build();
    draw.add_rect(min, [min[0] + 3.0, max[1]], palette.accent)
        .filled(true)
        .build();
}

fn draw_tree_entry_icons(
    ui: &Ui,
    row_x: f32,
    group: bool,
    geolocated: bool,
    has_parent: bool,
    palette: &Palette,
) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let x = tree_icon_x(ui, row_x, group, has_parent);
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
        let center = [x + 24.0, y];
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

fn tree_icon_x(ui: &Ui, row_x: f32, group: bool, has_parent: bool) -> f32 {
    // Only nested leaves include a tree indent that their parent folder icon
    // does not. Root leaves share the same icon column as root folders.
    row_x + ui.text_line_height() + 4.0
        - if !group && has_parent {
            ui.clone_style().indent_spacing()
        } else {
            0.0
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
    total: u128,
    decimals: usize,
    palette: &Palette,
) {
    let value = format_count_percentage(count, total, decimals);
    detail_row_tinted(ui, name, &value, special.color(palette));
}
pub(super) fn format_count_percentage(count: u128, total: u128, decimals: usize) -> String {
    let percent = if total == 0 {
        0.0
    } else {
        100.0 * count as f64 / total as f64
    };
    format!("{count} ({percent:.decimals$}%)")
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
    let vertical_padding = ui.clone_style().cell_padding()[1];
    table_extent_with_row_height(
        ui,
        content_width,
        rows,
        padding,
        ui.text_line_height() + 2.0 * vertical_padding,
    )
}

pub(super) fn table_extent_with_row_height(
    ui: &Ui,
    content_width: f32,
    rows: usize,
    padding: f32,
    row_height: f32,
) -> ([f32; 2], bool) {
    let available = ui.content_region_avail();
    let vertical_padding = ui.clone_style().cell_padding()[1];
    let content_height = ui.frame_height() + rows as f32 * row_height + 2.0 * vertical_padding;
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
