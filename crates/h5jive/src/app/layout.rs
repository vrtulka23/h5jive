use super::*;

fn volume_key_cells(key: &VolumeKey) -> usize {
    key.slice
        .axes
        .iter()
        .flatten()
        .fold(1usize, |total, &axis| {
            total.saturating_mul(key.slice.ranges[axis][1] - key.slice.ranges[axis][0] + 1)
        })
}

impl App {
    pub(super) fn draw(&mut self, ui: &Ui, plot_ui: &PlotUi<'_>, plot_ctx: &PlotContext) {
        self.poll_cell_edit_job();
        self.poll_file_open_jobs();
        if self.plugins.take_reload().is_some() {
            if self.sync_restore_selection.is_none() {
                self.sync_restore_selection = self.selection.clone();
            }
            self.pending_browser_b = self.file_paths.get(1).cloned();
            if let Some(path) = self.file_paths.first() {
                self.file_path_inputs[0] = path.display().to_string();
                self.load_file_slot(0);
            }
        }
        self.dataset_browser.poll(&self.settings.dataset_sources);
        let palette = self.settings.palette.clone();
        let control_bg = mix_color(palette.panel, [1.0, 1.0, 1.0, 1.0], 0.055);
        let control_hover = mix_color(control_bg, palette.accent, 0.25);
        let control_active = mix_color(control_bg, palette.accent, 0.38);
        let accent_active = mix_color(palette.accent, palette.text, 0.18);
        let _theme_colors = ThemeColors(
            [
                (StyleColor::Text, palette.text),
                (StyleColor::TextDisabled, palette.muted),
                (StyleColor::WindowBg, palette.background),
                (StyleColor::ChildBg, palette.background),
                (StyleColor::PopupBg, palette.panel),
                (StyleColor::MenuBarBg, palette.panel),
                (StyleColor::TitleBg, palette.panel),
                (StyleColor::TitleBgActive, palette.panel),
                (StyleColor::FrameBg, control_bg),
                (StyleColor::FrameBgHovered, control_hover),
                (StyleColor::FrameBgActive, control_active),
                (StyleColor::Button, control_bg),
                (StyleColor::ButtonHovered, control_hover),
                (StyleColor::ButtonActive, palette.accent),
                (StyleColor::CheckMark, palette.accent),
                (StyleColor::CheckboxSelectedBg, control_active),
                (StyleColor::SliderGrab, palette.accent),
                (StyleColor::SliderGrabActive, accent_active),
                (StyleColor::Header, palette.panel),
                (StyleColor::HeaderHovered, palette.view_hover),
                (StyleColor::HeaderActive, palette.accent),
                (StyleColor::Tab, palette.panel),
                (StyleColor::TabHovered, palette.view_hover),
                (StyleColor::TabSelected, palette.accent),
                (StyleColor::TableHeaderBg, palette.panel),
                (StyleColor::TableRowBg, palette.background),
                (StyleColor::TableRowBgAlt, palette.table_row_alt),
                (StyleColor::Border, palette.border),
                (StyleColor::TableBorderStrong, palette.border),
                (StyleColor::TableBorderLight, palette.border),
                (StyleColor::Separator, palette.border),
                (StyleColor::ScrollbarBg, palette.background),
                (StyleColor::ScrollbarGrab, control_bg),
                (StyleColor::ScrollbarGrabHovered, control_hover),
                (StyleColor::ScrollbarGrabActive, palette.accent),
                (StyleColor::ResizeGrip, control_hover),
                (StyleColor::ResizeGripHovered, palette.accent),
                (StyleColor::ResizeGripActive, accent_active),
                (StyleColor::SeparatorHovered, palette.accent),
                (StyleColor::SeparatorActive, accent_active),
                (StyleColor::TextSelectedBg, control_active),
                (StyleColor::InputTextCursor, palette.accent),
                (StyleColor::NavCursor, palette.accent),
            ]
            .into_iter()
            .map(|(name, color)| ui.push_style_color(name, color))
            .collect(),
        );
        let rounding = self.settings.profiles[self.settings.active_profile].rounding;
        let _window_rounding = ui.push_style_var(StyleVar::WindowRounding(rounding));
        let _child_rounding = ui.push_style_var(StyleVar::ChildRounding(rounding));
        let _frame_rounding = ui.push_style_var(StyleVar::FrameRounding(rounding));
        let _popup_rounding = ui.push_style_var(StyleVar::PopupRounding(rounding));
        let _tab_rounding = ui.push_style_var(StyleVar::TabRounding(rounding));
        let _grab_rounding = ui.push_style_var(StyleVar::GrabRounding(rounding));
        let _scrollbar_rounding = ui.push_style_var(StyleVar::ScrollbarRounding(rounding));
        let _cell_padding = ui.push_style_var(StyleVar::CellPadding(TABLE_CELL_PADDING));
        let _panel_padding = ui.push_style_var(StyleVar::WindowPadding(PANEL_PADDING));
        let _item_spacing = ui.push_style_var(StyleVar::ItemSpacing([PANEL_GAP, PANEL_GAP]));
        let _frame_padding = ui.push_style_var(StyleVar::FramePadding(CONTROL_FRAME_PADDING));
        let _inner_spacing = ui.push_style_var(StyleVar::ItemInnerSpacing(CONTROL_INNER_SPACING));
        self.resource_monitor.sample();
        let size = ui.io().display_size();
        let tutorial_tips = self.settings.tutorial_tips;
        let menu_height = ui
            .main_menu_bar(|| {
                if let Some(_menu) = ui.begin_menu("Files") {
                    ui.text("Input file A");
                    ui.set_next_item_width(360.0);
                    ui.input_text("##input_file_a", &mut self.file_path_inputs[0])
                        .build();
                    item_tip(ui, tutorial_tips, "", "Paste a local file path, then choose Load A.");
                    if ui.button("Browse A...") {
                        self.open_file_picker(0);
                    }
                    item_tip(ui, tutorial_tips, "", "Select a file, then double-click it to open.");
                    ui.same_line();
                    if ui.button("Load A") {
                        self.load_file_slot(0);
                    }
                    item_tip(ui, tutorial_tips, "", "File A is the primary file for browsing and comparisons.");
                    ui.separator();
                    ui.text("Input file B");
                    ui.set_next_item_width(360.0);
                    ui.input_text("##input_file_b", &mut self.file_path_inputs[1])
                        .build();
                    item_tip(ui, tutorial_tips, "", "Paste a second file path to compare with File A.");
                    if ui.button("Browse B...") {
                        self.open_file_picker(1);
                    }
                    item_tip(ui, tutorial_tips, "", "Select a file, then double-click it to open.");
                    ui.same_line();
                    if ui.button("Load B") {
                        self.load_file_slot(1);
                    }
                    item_tip(ui, tutorial_tips, "", "Load A first, then B for comparisons.");
                    if let Some(error) = &self.file_error {
                        ui.separator();
                        ui.text_wrapped(error);
                    }
                    ui.separator();
                    if self.workspace_mode == WorkspaceMode::DataInspector
                        && ui.menu_item("Save current view...")
                    {
                        self.open_export_dialog();
                    }
                    if self.workspace_mode == WorkspaceMode::DataInspector {
                        item_tip(ui, tutorial_tips, "", "Export the active view as an image.");
                    }
                }
                if let Some(_menu) = ui.begin_menu("Appearance") {
                    for index in 0..self.settings.profiles.len() {
                        let selected = index == self.settings.active_profile;
                        let name = &self.settings.profiles[index].name;
                        let title = match name.as_str() {
                            "dark" => "Dark",
                            "light" => "Light",
                            "high_contrast" => "High contrast",
                            "soft_slate" => "Soft Slate",
                            _ => name,
                        };
                        let label = format!("{} {title}", if selected { "->" } else { "  " });
                        if ui.menu_item(&label) {
                            self.settings.active_profile = index;
                            self.settings.palette = self.settings.profiles[index].palette.clone();
                            self.heatmap_cache = None;
                        }
                        item_tip(ui, tutorial_tips, "", "Choose a color theme for the interface.");
                    }
                }
                if let Some(_menu) = ui.begin_menu("Help") {
                    if ui.menu_item("Help for current section...") {
                        self.show_section_help = true;
                    }
                    item_tip(ui, tutorial_tips, "", "Open guidance for the active view.");
                    ui.separator();
                    if ui.menu_item_enabled_selected_no_shortcut(
                        "Tutorial tips",
                        self.settings.tutorial_tips,
                        true,
                    ) {
                        self.tutorial_tips_error = self
                            .settings
                            .set_tutorial_tips(!self.settings.tutorial_tips)
                            .err()
                            .map(|error| format!("Could not save Tutorial tips: {error}"));
                    }
                    item_tip(ui, tutorial_tips, "Show extra guidance when hovering controls. This choice is saved in DIPL overrides.", "Uncheck this to keep only normal tooltips.");
                    if let Some(error) = &self.tutorial_tips_error {
                        ui.text_wrapped(error);
                    }
                    ui.separator();
                    if ui.menu_item("About H5Jive") {
                        self.show_about = true;
                    }
                    item_tip(ui, tutorial_tips, "", "See the installed version and maintainer.");
                }
                ui.same_line();
                let display_label = self
                    .display_backend
                    .map(|backend| format!("  ·  Display {backend}"))
                    .unwrap_or_default();
                let mut label = format!("{}{display_label}", self.resource_monitor.label(true));
                let padding = ui.clone_style().window_padding()[0];
                let mut x = ui.window_size()[0] - padding - ui.calc_text_size(&label)[0];
                if x <= ui.cursor_pos_x() + 16.0 {
                    label = format!("{}{display_label}", self.resource_monitor.label(false));
                    x = ui.window_size()[0] - padding - ui.calc_text_size(&label)[0];
                }
                if x > ui.cursor_pos_x() + 16.0 {
                    ui.set_cursor_pos_x(x);
                    ui.text_colored(palette.muted, label);
                    item_plain_tip(ui,
                        "Raw: uncompressed byte size of the selected dataset, not its measured memory use.\n\
                         RAM: resident memory used by the whole H5Jive process (CPU memory).\n\
                         UI: average time spent building a UI frame during the last second; excludes GPU rendering.\n\
                         Display: window system in use. On macOS, AppKit creates windows managed by WindowServer. XWayland is X11 inside a Wayland session.\n\
                         Compare RAM before and after loading or processing a dataset to observe its impact.",
                    );
                }
                ui.window_size()[1]
            })
            .unwrap_or(0.0);
        let flags = WindowFlags::NO_MOVE
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_SAVED_SETTINGS;
        let mode_bar_height = 44.0;
        {
            let _mode_bar_padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
            let _mode_bar_spacing = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
            ui.window("##workspace_mode_bar")
                .position([0.0, menu_height], Condition::Always)
                .size([size[0], mode_bar_height], Condition::Always)
                .flags(flags | WindowFlags::NO_DECORATION | WindowFlags::NO_SCROLLBAR)
                .build(|| {
                    let total_width = ui.content_region_avail()[0];
                    let tab_width = total_width / WorkspaceMode::ALL.len() as f32;
                    for (index, mode) in WorkspaceMode::ALL.into_iter().enumerate() {
                        if index > 0 {
                            ui.same_line_with_spacing(0.0, 0.0);
                        }
                        let width = if index == WorkspaceMode::ALL.len() - 1 {
                            total_width - tab_width * index as f32
                        } else {
                            tab_width
                        };
                        let clicked = ui.invisible_button(
                            format!("##workspace_mode_{index}"),
                            [width, mode_bar_height],
                        );
                        let hovered = ui.is_item_hovered();
                        let min = ui.item_rect_min();
                        let max = ui.item_rect_max();
                        let selected = self.workspace_mode == mode;
                        let background = if selected {
                            mix_color(palette.panel, palette.accent, 0.17)
                        } else if hovered {
                            palette.view_hover
                        } else {
                            palette.panel
                        };
                        let draw = ui.get_window_draw_list();
                        draw.add_rect(min, max, background).filled(true).build();
                        if selected {
                            draw.add_rect([min[0], max[1] - 3.0], max, palette.accent)
                                .filled(true)
                                .build();
                        }
                        if index > 0 {
                            draw.add_line(min, [min[0], max[1]], palette.border).build();
                        }
                        let label = mode.title();
                        let text_size = ui.calc_text_size(label);
                        let icon_size = 20.0;
                        let icon_gap = 8.0;
                        let group_width = icon_size + icon_gap + text_size[0];
                        let group_x = min[0] + (width - group_width) * 0.5;
                        let text_y = min[1] + (mode_bar_height - text_size[1]) * 0.5;
                        let foreground = if selected || hovered {
                            palette.text
                        } else {
                            palette.muted
                        };
                        draw_workspace_mode_icon(
                            &draw,
                            mode,
                            [group_x, min[1] + (mode_bar_height - icon_size) * 0.5],
                            if selected { palette.accent } else { foreground },
                        );
                        draw.add_text([group_x + icon_size + icon_gap, text_y], foreground, label);
                        if clicked {
                            self.workspace_mode = mode;
                        }
                        item_tip(
                            ui,
                            tutorial_tips,
                            "",
                            match mode {
                                WorkspaceMode::DatasetBrowser => {
                                    "Find products in configured dataset sources."
                                }
                                WorkspaceMode::DataInspector => {
                                    "Explore file contents and visualize datasets."
                                }
                                WorkspaceMode::PluginLab => {
                                    "Open optional analysis and comparison tools."
                                }
                            },
                        );
                    }
                });
        }
        let menu_height = menu_height + mode_bar_height;
        let top_menu_height = menu_height - mode_bar_height;
        let panel_height = (size[1] - menu_height).max(1.0);
        if self.workspace_mode == WorkspaceMode::DatasetBrowser {
            self.view_rect = [0.0; 4];
            self.draw_dataset_browser(ui, menu_height, panel_height, flags);
            self.draw_help_dialogs(ui);
            self.draw_file_picker(ui);
            self.draw_global_loading_overlay(ui, size, top_menu_height);
            self.draw_global_error_overlay(ui, size, menu_height);
            self.draw_file_drop_overlay(ui, size);
            return;
        }
        if self.workspace_mode != WorkspaceMode::DataInspector {
            self.view_rect = [0.0; 4];
            self.draw_workspace_placeholder(ui, plot_ui, menu_height, panel_height, flags);
            self.draw_help_dialogs(ui);
            self.draw_file_picker(ui);
            self.draw_global_loading_overlay(ui, size, top_menu_height);
            self.draw_global_error_overlay(ui, size, menu_height);
            self.draw_file_drop_overlay(ui, size);
            return;
        }
        let side = self.sidebar_width.clamp(
            self.settings.sidebar_min_width,
            (size[0] - self.settings.viewer_min_width)
                .min(self.settings.sidebar_max_width)
                .max(self.settings.sidebar_min_width),
        );
        ui.window("Input files and dataset tree")
            .position([0.0, menu_height], Condition::Always)
            .size([side, panel_height], Condition::Always)
            .flags(flags | WindowFlags::NO_TITLE_BAR | WindowFlags::NO_SCROLLBAR)
            .build(|| {
                let selector_reserve = self.view_selector_height;
                let panel_bottom =
                    ui.cursor_pos_y() + ui.content_region_avail()[1] - selector_reserve;
                let panel_space = (panel_bottom
                    - ui.cursor_pos_y()
                    - self.settings.splitter_width
                    - 2.0 * PANEL_GAP)
                    .max(1.0);
                let minimum = self.settings.panel_min_height.min(panel_space * 0.5);
                let details_height = (panel_space * self.details_panel_fraction)
                    .clamp(minimum, (panel_space - minimum).max(minimum));
                let tree_height = (panel_space - details_height).max(1.0);
                let prior_selection = self.selection.clone();
                let mut reload_requested = false;
                ui.child_window("file_tree")
                    .size([0.0, tree_height])
                    .build(ui, || {
                        if let Some(_tabs) = ui.tab_bar("file_tree_tabs") {
                            for index in 0..2 {
                                let label = if index == 0 { "File A" } else { "File B" };
                                let tab_flags = if self.startup_tab == Some(index) {
                                    TabItemFlags::SET_SELECTED
                                } else {
                                    TabItemFlags::NONE
                                };
                                if let Some(_tab) = ui.tab_item_with_flags(label, None, tab_flags) {
                                    item_tip(ui, self.settings.tutorial_tips, "", "Switch between the two loaded file trees.");
                                    ui.child_window(format!("file_tree_slot_{index}"))
                                        .size([0.0, 0.0])
                                        .build(ui, || {
                                            if self.cell_edit_job.is_some() {
                                                ui.text("Applying cell changes...");
                                            } else if let Some(file) = self.files.get(index) {
                                                draw_file_root(
                                                    ui,
                                                    file,
                                                    index,
                                                    &mut self.selection,
                                                    self.settings.tutorial_tips,
                                                    &self.settings.palette,
                                                );
                                                ui.separator();
                                                for entry in &file.tree.children {
                                                    reload_requested |= draw_entry(
                                                        ui,
                                                        entry,
                                                        file,
                                                        index,
                                                        false,
                                                        &mut self.selection,
                                                        &mut self.tree_geolocation_cache,
                                                        &self.settings.dataset_rules,
                                                        self.settings.show_dataset_shape,
                                                        self.settings.tutorial_tips,
                                                        &self.settings.palette,
                                                    );
                                                }
                                            } else {
                                                ui.text_wrapped(format!(
                                                    "Load {label} from the Files menu or Dataset browser."
                                                ));
                                            }
                                        });
                                }
                            }
                            self.startup_tab = None;
                        }
                    });
                if reload_requested && prior_selection == self.selection {
                    self.reload_selected_dataset();
                }
                if let (Some(previous), Some(requested)) =
                    (prior_selection, self.selection.clone())
                {
                    if (previous.file != requested.file || previous.path != requested.path)
                        && self.pending_cell_edits.keys().any(|key| {
                            key.file == previous.file && key.path == previous.path
                        })
                    {
                        self.selection = Some(previous);
                        self.queued_selection = Some(requested);
                    }
                }
                self.refresh_selection();
                let delta = draw_splitter(
                    ui,
                    "##tree_details_splitter",
                    [ui.content_region_avail()[0], self.settings.splitter_width],
                    false,
                    &self.settings.palette,
                );
                self.details_panel_fraction =
                    (self.details_panel_fraction - delta / panel_space).clamp(0.0, 1.0);
                self.draw_details_panel(ui, details_height);
                let gap = (panel_bottom
                    - ui.cursor_pos_y()
                    - ui.clone_style().item_spacing()[1])
                    .max(0.0);
                if gap > 0.0 {
                    ui.dummy([1.0, gap]);
                }
                let selector_start = ui.cursor_pos_y();
                self.draw_view_selector(ui);
                self.view_selector_height = (ui.cursor_pos_y() - selector_start).max(1.0);
            });
        {
            let _padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
            let _minimum = ui.push_style_var(StyleVar::WindowMinSize([1.0, 1.0]));
            ui.window("##main_splitter_window")
                .position([side, menu_height], Condition::Always)
                .size(
                    [self.settings.splitter_width, panel_height],
                    Condition::Always,
                )
                .flags(flags | WindowFlags::NO_DECORATION | WindowFlags::NO_SCROLLBAR)
                .build(|| {
                    let delta = draw_splitter(
                        ui,
                        "##main_splitter",
                        [self.settings.splitter_width, panel_height],
                        true,
                        &self.settings.palette,
                    );
                    self.sidebar_width = (side + delta).clamp(
                        self.settings.sidebar_min_width,
                        (size[0] - self.settings.viewer_min_width)
                            .min(self.settings.sidebar_max_width)
                            .max(self.settings.sidebar_min_width),
                    );
                });
        }
        let side = self.sidebar_width.clamp(
            self.settings.sidebar_min_width,
            (size[0] - self.settings.viewer_min_width)
                .min(self.settings.sidebar_max_width)
                .max(self.settings.sidebar_min_width),
        ) + self.settings.splitter_width;
        self.view_rect = [side, menu_height, (size[0] - side).max(1.0), panel_height];
        ui.window("###viewing_window")
            .position([side, menu_height], Condition::Always)
            .size([(size[0] - side).max(1.0), panel_height], Condition::Always)
            .flags(flags | WindowFlags::NO_TITLE_BAR)
            .build(|| {
                let available = ui.content_region_avail();
                let _wrapper_padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
                ui.child_window("##active_view_content")
                    .size([0.0, available[1]])
                    .build(ui, || {
                        if self.view == ViewMode::Spatial3d {
                            self.draw_volume_view(ui, plot_ui, plot_ctx);
                            return;
                        }
                        if self.view == ViewMode::Profiles1d {
                            self.draw_profile_view(ui);
                            return;
                        }
                        if self.view == ViewMode::WorldMap {
                            self.draw_world_map_view(ui, plot_ctx);
                            return;
                        }
                        if self.view == ViewMode::Globe {
                            self.draw_globe_view(ui, plot_ctx);
                            return;
                        }
                        if !matches!(self.view, ViewMode::Data | ViewMode::Slice2d) {
                            self.draw_view_placeholder(ui);
                            return;
                        }
                        self.draw_data_view(ui, plot_ui, plot_ctx);
                    });
            });
        self.draw_help_dialogs(ui);
        self.draw_pending_selection_dialog(ui);
        self.draw_file_picker(ui);
        self.draw_export_dialog(ui);
        self.draw_global_loading_overlay(ui, size, top_menu_height);
        self.draw_global_error_overlay(ui, size, menu_height);
        self.draw_file_drop_overlay(ui, size);
    }

    fn draw_file_drop_overlay(&self, ui: &Ui, size: [f32; 2]) {
        if !self.file_drop.hovering {
            return;
        }
        let draw = ui.get_foreground_draw_list();
        draw.add_rect([0.0, 0.0], size, [0.0, 0.0, 0.0, 0.68])
            .filled(true)
            .build();
        let margin = 24.0;
        let gap = 16.0;
        let half_width = ((size[0] - margin * 2.0 - gap) * 0.5).max(1.0);
        let selected = drop_slot(self.file_drop.pointer_x, size[0]);
        for slot in 0..2 {
            let min = [margin + slot as f32 * (half_width + gap), margin];
            let max = [min[0] + half_width, (size[1] - margin).max(margin + 1.0)];
            let active = selected == Some(slot);
            let fill = if active {
                mix_color(
                    self.settings.palette.panel,
                    self.settings.palette.accent,
                    0.30,
                )
            } else {
                self.settings.palette.panel
            };
            draw.add_rect(min, max, fill)
                .filled(true)
                .rounding(12.0)
                .build();
            draw.add_rect(
                min,
                max,
                if active {
                    self.settings.palette.accent
                } else {
                    self.settings.palette.border
                },
            )
            .rounding(12.0)
            .thickness(if active { 3.0 } else { 1.0 })
            .build();
            let label = if slot == 0 {
                "Drop as File A"
            } else {
                "Drop as File B"
            };
            let text_size = ui.calc_text_size(label);
            draw.add_text(
                [
                    (min[0] + max[0] - text_size[0]) * 0.5,
                    (min[1] + max[1] - text_size[1]) * 0.5,
                ],
                self.settings.palette.text,
                label,
            );
        }
    }

    fn draw_global_loading_overlay(&self, ui: &Ui, size: [f32; 2], menu_height: f32) {
        let mut tasks = Vec::new();
        for (slot, job) in self.file_open_jobs.iter().enumerate() {
            if job.is_some() {
                tasks.push(if slot == 0 { "file A" } else { "file B" });
            }
        }
        if let Some(label) = self.map_loading_label() {
            tasks.push(label);
        }
        if let Some(label) = self.heatmap_tile_loading_label() {
            tasks.push(label);
        }
        if let Some(label) = self.volume_loading_label() {
            tasks.push(label);
        }
        if let Some(label) = self.volume_raster_loading_label() {
            tasks.push(label);
        }
        if self.profile_job.is_some() {
            let has_overview = self.profile_cache.as_ref().is_some_and(|(_, result)| {
                result
                    .as_ref()
                    .is_ok_and(|data| data.sample_positions.is_some())
            });
            tasks.push(if has_overview {
                "1D profile detail"
            } else {
                "1D profile"
            });
        }
        for (active, label) in [
            (self.heatmap_overview_job.is_some(), "2D image"),
            (self.heatmap_range_job.is_some(), "2D range"),
            (self.statistics_job.is_some(), "stats"),
            (self.distribution_job.is_some(), "distribution"),
        ] {
            if active {
                tasks.push(label);
            }
        }
        tasks.extend(self.dataset_browser.active_task_labels());
        if !tasks.is_empty() {
            let mut grouped: Vec<(&str, usize)> = Vec::new();
            for task in tasks {
                if let Some((_, count)) = grouped.iter_mut().find(|(label, _)| *label == task) {
                    *count += 1;
                } else {
                    grouped.push((task, 1));
                }
            }
            let mut label = String::from("Loading ");
            for (index, (task, count)) in grouped.iter().take(2).enumerate() {
                if index > 0 {
                    label.push_str(" + ");
                }
                label.push_str(task);
                if *count > 1 {
                    label.push_str(&format!(" ({count})"));
                }
            }
            let remaining: usize = grouped.iter().skip(2).map(|(_, count)| count).sum();
            if remaining > 0 {
                label.push_str(&format!(" +{remaining}"));
            }
            draw_loading_indicator(
                ui,
                [size[0] * 0.5, menu_height * 0.5],
                &label,
                &self.settings.palette,
            );
        }
    }

    fn draw_global_error_overlay(&self, ui: &Ui, size: [f32; 2], menu_height: f32) {
        if self.workspace_mode != WorkspaceMode::DataInspector || self.view != ViewMode::Profiles1d
        {
            return;
        }
        if let Some(error) = self.profile_index_error() {
            draw_error_indicator(
                ui,
                [size[0] - 10.0, menu_height + 18.0],
                &format!("Profile indexes: {error}"),
                &self.settings.palette,
            );
        }
    }

    fn profile_index_error(&self) -> Option<String> {
        let draft = self.draft_profile.as_ref()?;
        let axis = draft.y_axis?;
        let selection = self.selection.as_ref()?;
        let entry = find_entry(&self.files[selection.file].tree, &selection.path)?;
        let Kind::Dataset { shape, .. } = &entry.kind else {
            return None;
        };
        parse_profile_indexes(&draft.y_indexes, *shape.get(axis)?).err()
    }

    fn heatmap_tile_loading_label(&self) -> Option<&'static str> {
        let (working, _) = self.heatmap_tile_job.as_ref()?;
        let previous = self
            .heatmap_cache
            .as_ref()
            .filter(|(_, result)| result.is_ok())
            .map(|(key, _)| key)
            .or_else(|| self.heatmap_tiles.last().map(|(key, _)| key));
        let Some(previous) = previous.filter(|previous| {
            previous.file == working.file
                && previous.path == working.path
                && previous.slice == working.slice
        }) else {
            return Some(
                if self
                    .heatmap_overview_cache
                    .as_ref()
                    .is_some_and(|(key, result)| {
                        key.file == working.file
                            && key.path == working.path
                            && key.slice == working.slice
                            && result.is_ok()
                    })
                {
                    "2D tile (refining resolution)"
                } else {
                    "2D tile"
                },
            );
        };
        let area =
            |bounds: [usize; 4]| (bounds[1] - bounds[0]).saturating_mul(bounds[3] - bounds[2]);
        Some(match area(working.bounds).cmp(&area(previous.bounds)) {
            std::cmp::Ordering::Less => "2D tile (refining resolution)",
            std::cmp::Ordering::Greater => "2D tile (reducing detail)",
            std::cmp::Ordering::Equal => "2D region",
        })
    }

    fn volume_loading_label(&self) -> Option<&'static str> {
        let (working, _) = self.volume_job.as_ref()?;
        let Some((previous, Ok(_))) = self.volume_cache.as_ref().filter(|(key, _)| {
            key.file == working.file
                && key.path == working.path
                && key.slice.axes == working.slice.axes
        }) else {
            return Some("3D volume");
        };
        Some(
            match volume_key_cells(working).cmp(&volume_key_cells(previous)) {
                std::cmp::Ordering::Less => "3D volume (refining resolution)",
                std::cmp::Ordering::Greater => "3D volume (reducing detail)",
                std::cmp::Ordering::Equal => "3D region",
            },
        )
    }

    fn volume_raster_loading_label(&self) -> Option<&'static str> {
        let working = &self.volume_raster_job.as_ref()?.key;
        let Some((previous, _)) = self.volume_raster.as_ref().filter(|(key, _)| {
            key.volume.file == working.volume.file && key.volume.path == working.volume.path
        }) else {
            return Some("3D render");
        };
        let smallest_layer =
            |key: &VolumeRasterKey| key.layer_keys.iter().map(volume_key_cells).min();
        Some(if working.scale[0] > previous.scale[0] * 1.001 {
            "3D render (refining resolution)"
        } else if working.scale[0] < previous.scale[0] * 0.999 {
            "3D render (reducing detail)"
        } else if smallest_layer(working) < smallest_layer(previous) {
            "3D render (refining resolution)"
        } else if smallest_layer(working) > smallest_layer(previous) {
            "3D render (reducing detail)"
        } else {
            "3D render"
        })
    }

    fn map_loading_label(&self) -> Option<&'static str> {
        let (working, _) = self.map_job.as_ref()?;
        let Some((previous, Ok(_))) = self.map_cache.as_ref().filter(|(key, _)| {
            key.file == working.file
                && key.path == working.path
                && key.time == working.time
                && key.spectral == working.spectral
        }) else {
            return Some("map");
        };
        Some(match working.max_points.cmp(&previous.max_points) {
            std::cmp::Ordering::Greater => "map (refining resolution)",
            std::cmp::Ordering::Less => "map (reducing detail)",
            std::cmp::Ordering::Equal => "map",
        })
    }
    fn draw_workspace_placeholder(
        &mut self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        top: f32,
        height: f32,
        flags: WindowFlags,
    ) {
        ui.window(format!(
            "{}###workspace_content",
            self.workspace_mode.title()
        ))
        .position([0.0, top], Condition::Always)
        .size([ui.io().display_size()[0], height], Condition::Always)
        .flags(flags)
        .build(|| match self.workspace_mode {
            WorkspaceMode::DatasetBrowser => {
                if ui.button("Open data inspector") {
                    self.workspace_mode = WorkspaceMode::DataInspector;
                }
            }
            WorkspaceMode::PluginLab => {
                let can_write_files = self.cell_edit_job.is_none()
                    && !self.plugins.is_syncing()
                    && self.file_open_jobs.iter().all(Option::is_none)
                    && self.pending_cell_edits.is_empty();
                let write_allowed = [can_write_files; 2];
                if let Some(selection) = self.plugins.draw(
                    ui,
                    plot_ui,
                    &mut self.files,
                    &self.file_paths,
                    &self.settings,
                    self.selection.as_ref(),
                    write_allowed,
                ) {
                    if let Some(current) = &self.selection {
                        if (current.file != selection.file || current.path != selection.path)
                            && self
                                .pending_cell_edits
                                .keys()
                                .any(|key| key.file == current.file && key.path == current.path)
                        {
                            self.queued_selection = Some(selection);
                        } else {
                            self.selection = Some(selection);
                        }
                    } else {
                        self.selection = Some(selection);
                    }
                    self.workspace_mode = WorkspaceMode::DataInspector;
                    self.switch_view(ViewMode::Data);
                }
                if self.plugins.has_sync_request() {
                    if !self.pending_cell_edits.is_empty() || self.cell_edit_job.is_some() {
                        self.plugins.invalidate();
                        self.file_error = Some(
                            "Apply or discard pending cell edits before synchronizing files."
                                .into(),
                        );
                    } else if self.file_open_jobs.iter().all(Option::is_none) {
                        self.sync_restore_selection = self.selection.clone();
                        self.reset_file_selection();
                        self.files.clear();
                        self.plugins.start_requested_sync(&self.file_paths);
                    }
                }
            }
            WorkspaceMode::DataInspector => {}
        });
    }
    fn current_help_topic(&self) -> (&'static str, &'static str) {
        match self.workspace_mode {
            WorkspaceMode::DatasetBrowser => (
                "Dataset browser",
                "Browse products indexed from the local dataset sources configured in DIPL. Open a product as file A or B, then inspect its datasets or use it in Plugin Lab.",
            ),
            WorkspaceMode::PluginLab => self.plugins.help_topic(&self.settings),
            WorkspaceMode::DataInspector => match self.view {
                ViewMode::Spatial3d => (
                    "3D projection",
                    "Preview a numeric dataset in 3D. Choose displayed dimensions and pin any remaining dimensions in the controls. Use the projection and range controls to focus the volume.",
                ),
                ViewMode::Slice2d => (
                    "2D slice",
                    "Choose two dataset dimensions for the heatmap and pin the others. Pan and zoom the plot to inspect cells; the view loads bounded regions of large datasets.",
                ),
                ViewMode::Profiles1d => (
                    "1D profiles",
                    "Choose a profile dimension and pin the remaining dimensions. The plot shows values along the selected axis; adjust the controls to compare other slices. Long profiles can first show a sampled overview while the full line loads. The badge reports sampled positions per line and changes to Settled when the full read finishes.",
                ),
                ViewMode::WorldMap => (
                    "World map",
                    "Display geolocated observations on a flat map. Select a dataset with usable coordinates, then use the map controls to navigate and adjust the display.",
                ),
                ViewMode::Globe => (
                    "Globe",
                    "Display geolocated observations on a rotatable globe. Select a dataset with usable coordinates, then rotate and zoom the globe to inspect its coverage.",
                ),
                ViewMode::Data => (
                    "Data",
                    "Select a group or dataset in the file tree to inspect its values and metadata. Click a numeric HDF5 or NetCDF-4 table cell, type a new value, and press Enter to stage it. Type Fill (or fill) to use the dataset's _FillValue, or NaN, +Inf, or -Inf for floating-point datasets; these words are case-insensitive. Changed cells have an orange highlight. The bar between the table and slice controls lets you Apply or Discard pending edits. Staged edits remain attached to their dataset coordinates while you change slices; apply them before closing H5Jive. Numeric datasets can also be viewed as a 2D slice, 1D profile, 3D projection, or geospatial view when coordinates are available.",
                ),
            },
        }
    }

    fn draw_help_dialogs(&mut self, ui: &Ui) {
        const SECTION_HELP_POPUP: &str = "Current section help##section_help";
        if self.show_section_help {
            ui.open_popup(SECTION_HELP_POPUP);
            self.show_section_help = false;
        }
        if let Some(_popup) = ui
            .begin_modal_popup_config(SECTION_HELP_POPUP)
            .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
            .begin()
        {
            let (title, help) = self.current_help_topic();
            ui.text(title);
            ui.separator();
            ui.child_window("section_help_text")
                .size([500.0, 170.0])
                .build(ui, || {
                    ui.text_wrapped(help);
                    ui.spacing();
                    ui.text_wrapped("Tutorial tips add extra guidance when you hover controls. Turn them on or off in Help → Tutorial tips; the choice is saved for future launches.");
                    if self.workspace_mode == WorkspaceMode::DataInspector {
                        ui.spacing();
                        ui.text("Inspector details");
                        ui.text_wrapped("Statistics exclude fill and nonfinite values. Large-dataset quartiles may use a bounded sample and are marked approximate in the table.");
                    }
                });
            if ui.button("Close##section_help") {
                ui.close_current_popup();
            }
        }
        self.draw_about_dialog(ui);
    }

    fn draw_about_dialog(&mut self, ui: &Ui) {
        if self.show_about {
            ui.window("About H5Jive")
                .size([440.0, 150.0], Condition::FirstUseEver)
                .opened(&mut self.show_about)
                .build(|| {
                    ui.text(format!("H5Jive v{}", env!("CARGO_PKG_VERSION")));
                    ui.separator();
                    ui.text("Author and maintainer: Ondrej Pêgo Jaura");
                    ui.text_wrapped("Dig into your data.");
                });
        }
    }
    pub(super) fn draw_view_selector(&mut self, ui: &Ui) {
        let dataset_available = self.selection.as_ref().is_some_and(|selection| {
            self.files.get(selection.file).is_some_and(|file| {
                find_entry(&file.tree, &selection.path)
                    .is_some_and(|entry| matches!(entry.kind, Kind::Dataset { .. }))
            })
        });
        let geo_available = self.selected_geo_source().is_some();
        let available = ui.content_region_avail()[0];
        let width = ((available - 5.0 * self.settings.view_button_gap) / 6.0).max(20.0);
        for (index, mode) in ViewMode::ALL.into_iter().enumerate() {
            if index > 0 {
                ui.same_line_with_spacing(0.0, self.settings.view_button_gap);
            }
            let clicked = ui.invisible_button(
                format!("##view_{}", mode.key()),
                [width, self.settings.view_button_height],
            );
            let hovered = ui.is_item_hovered();
            let min = ui.item_rect_min();
            let max = ui.item_rect_max();
            let enabled = if matches!(mode, ViewMode::WorldMap | ViewMode::Globe) {
                geo_available
            } else {
                dataset_available
            };
            if clicked && enabled {
                self.switch_view(mode);
            }
            draw_view_icon(
                ui,
                mode,
                min,
                max,
                self.view == mode,
                hovered && enabled,
                &self.settings.palette,
            );
            if !enabled {
                let overlay = self.settings.palette.panel;
                ui.get_window_draw_list()
                    .add_rect(min, max, [overlay[0], overlay[1], overlay[2], 0.70])
                    .filled(true)
                    .build();
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    if matches!(mode, ViewMode::WorldMap | ViewMode::Globe) {
                        "Select a geolocated numeric dataset to enable this view"
                    } else {
                        "Select a dataset from the tree to enable this view"
                    },
                    "Choose a numeric dataset in the tree first.",
                );
            } else {
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    &format!("{} - {}", mode.title(), mode.description()),
                    "Click to switch views without changing the selected dataset.",
                );
            }
        }
    }
    pub(super) fn draw_view_placeholder(&self, ui: &Ui) {
        ui.text(self.view.description());
        ui.spacing();
        match &self.selection {
            Some(selection) => {
                let file = &self.files[selection.file];
                ui.text(format!(
                    "File {}: {}",
                    if selection.file == 0 { "A" } else { "B" },
                    file.name
                ));
                ui.text(format!("Path: {}", selection.path));
                if let Some(entry) = find_entry(&file.tree, &selection.path) {
                    if let Kind::Dataset { shape, .. } = &entry.kind {
                        ui.text(format!("Shape: {shape:?}"));
                    }
                }
            }
            None => ui.text_wrapped("Select a dataset from the tree."),
        }
        ui.separator();
        ui.text_wrapped("The visualization renderer for this view is not connected yet.");
    }
}

fn make_file_picker_state(slot: usize, directory: PathBuf) -> FilePickerState {
    let mut state = FilePickerState {
        slot,
        directory: directory.clone(),
        entries: Vec::new(),
        selected: None,
        error: None,
    };
    match std::fs::read_dir(&directory) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        let is_directory = path.is_dir();
                        if !is_directory && (!path.is_file() || !is_supported_input_file(&path)) {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        state.entries.push(FilePickerEntry {
                            path,
                            label: name,
                            is_directory,
                        });
                    }
                    Err(error) => state.error = Some(error.to_string()),
                }
            }
            state.entries.sort_by(|left, right| {
                right
                    .is_directory
                    .cmp(&left.is_directory)
                    .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
            });
            if let Some(parent) = directory
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
            {
                state.entries.insert(
                    0,
                    FilePickerEntry {
                        path: parent.to_path_buf(),
                        label: "..".to_owned(),
                        is_directory: true,
                    },
                );
            }
        }
        Err(error) => state.error = Some(error.to_string()),
    }
    state
}

fn is_supported_input_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("nc" | "nc3" | "nc4" | "cdf" | "netcdf" | "h5" | "hdf5" | "hdf" | "he5" | "diph5")
    )
}

#[cfg(test)]
mod file_picker_tests {
    use super::is_supported_input_file;
    use std::path::Path;

    #[test]
    fn picker_accepts_netcdf_hdf5_and_diph5_extensions_case_insensitively() {
        for name in [
            "data.nc",
            "data.NC4",
            "data.cdf",
            "data.h5",
            "data.HDF5",
            "data.he5",
            "data.diph5",
        ] {
            assert!(is_supported_input_file(Path::new(name)), "{name}");
        }
        for name in ["notes.txt", "image.png", "data.hdf4", "no_extension"] {
            assert!(!is_supported_input_file(Path::new(name)), "{name}");
        }
    }
}

impl App {
    fn draw_dataset_browser(&mut self, ui: &Ui, top: f32, height: f32, flags: WindowFlags) {
        let mut open = None;
        let _browser_padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _browser_spacing = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
        ui.window("Dataset browser###workspace_content")
            .position([0.0, top], Condition::Always)
            .size([ui.io().display_size()[0], height], Condition::Always)
            .flags(flags | WindowFlags::NO_TITLE_BAR)
            .build(|| {
                let browser_height =
                    (ui.content_region_avail()[1] - self.browser_footer_height).max(120.0);
                open = self.dataset_browser.draw(
                    ui,
                    &self.settings.dataset_sources,
                    self.pending_browser_b.as_deref(),
                    &self.settings.palette,
                    browser_height,
                    self.settings.tutorial_tips,
                );
                let footer_start = ui.cursor_pos_y();
                let footer_top = ui.cursor_screen_pos()[1];
                ui.get_window_draw_list()
                    .add_line(
                        [0.0, footer_top],
                        [ui.io().display_size()[0], footer_top],
                        self.settings.palette.border,
                    )
                    .build();
                let _footer_padding =
                    ui.push_style_var(StyleVar::CellPadding(BROWSER_PANEL_PADDING));
                if let Some(_table) =
                    ui.begin_table_with_flags("##inspected_file_slots", 2, TableOptions::new())
                {
                    for slot in 0..2 {
                        ui.table_next_column();
                        let color = if slot == 0 {
                            self.settings.palette.accent
                        } else {
                            mix_color(
                                self.settings.palette.heatmap_low,
                                self.settings.palette.text,
                                0.38,
                            )
                        };
                        let label = if slot == 0 { "A" } else { "B" };
                        let loading = self.file_open_jobs[slot].is_some();
                        let path = self.file_open_paths[slot]
                            .as_ref()
                            .filter(|_| loading)
                            .or_else(|| self.file_paths.get(slot))
                            .or_else(|| {
                                (slot == 1)
                                    .then_some(self.pending_browser_b.as_ref())
                                    .flatten()
                            });
                        let status = if loading {
                            "Loading"
                        } else if slot == 1
                            && self.pending_browser_b.is_some()
                            && self.files.is_empty()
                        {
                            "Queued"
                        } else if path.is_some() {
                            "Loaded"
                        } else {
                            "Empty"
                        };
                        ui.text_colored(color, format!("File {label} · {status}"));
                        ui.text_wrapped(path.map_or_else(
                            || "No product selected".to_owned(),
                            |path| path.display().to_string(),
                        ));
                    }
                }
                if let Some(error) = &self.file_error {
                    ui.set_cursor_pos([BROWSER_PANEL_PADDING[0], ui.cursor_pos_y() + PANEL_GAP]);
                    ui.text_wrapped(error);
                }
                self.browser_footer_height = (ui.cursor_pos_y() - footer_start).max(1.0);
            });
        if let Some((slot, path)) = open {
            self.file_path_inputs[slot] = path.display().to_string();
            if slot == 1 && self.files.is_empty() {
                self.pending_browser_b = Some(path);
            } else {
                self.load_file_slot(slot);
            }
        }
    }
    fn open_file_picker(&mut self, slot: usize) {
        let current = PathBuf::from(self.file_path_inputs[slot].trim());
        let directory = if current.is_dir() {
            current
        } else {
            current
                .parent()
                .filter(|parent| parent.is_dir())
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("."))
        };
        self.file_picker = Some(make_file_picker_state(slot, directory));
    }

    fn draw_file_picker(&mut self, ui: &Ui) {
        let Some(picker) = self.file_picker.as_mut() else {
            return;
        };
        let slot_name = if picker.slot == 0 { "A" } else { "B" };
        let title = format!("Open File {slot_name}###file_picker");
        let mut opened = true;
        let mut navigate = None;
        let mut chosen = None;
        ui.window(title)
            .size([640.0, 520.0], Condition::FirstUseEver)
            .opened(&mut opened)
            .build(|| {
                if ui.button_with_size("##file_picker_refresh", [ui.frame_height(); 2]) {
                    navigate = Some(picker.directory.clone());
                }
                draw_file_picker_refresh_icon(ui, &self.settings.palette);
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "Refresh current folder",
                    "Rescan this folder if a file was added outside H5Jive.",
                );
                ui.same_line_with_spacing(0.0, PANEL_GAP);
                ui.align_text_to_frame_padding();
                ui.text_wrapped(picker.directory.display().to_string());
                if let Some(error) = &picker.error {
                    ui.text_wrapped(error);
                }
                ui.separator();
                ui.child_window("##file_picker_entries")
                    .size([0.0, 0.0])
                    .build(ui, || {
                        for entry in &picker.entries {
                            let selected = picker.selected.as_ref() == Some(&entry.path);
                            let icon_position = ui.cursor_screen_pos();
                            ui.dummy([17.0, ui.text_line_height()]);
                            draw_file_picker_icon(
                                ui,
                                icon_position,
                                entry.is_directory,
                                &self.settings.palette,
                            );
                            ui.same_line_with_spacing(0.0, 3.0);
                            let clicked = ui
                                .selectable_config(&entry.label)
                                .selected(selected)
                                .build();
                            let hovered = ui.is_item_hovered();
                            item_tip(
                                ui,
                                self.settings.tutorial_tips,
                                "",
                                if entry.is_directory {
                                    "Click to highlight; double-click to open this folder."
                                } else {
                                    "Click to highlight; double-click to open this file."
                                },
                            );
                            if clicked {
                                picker.selected = Some(entry.path.clone());
                            }
                            if hovered
                                && ui.is_mouse_double_clicked(dear_app::imgui::MouseButton::Left)
                            {
                                if entry.is_directory {
                                    navigate = Some(entry.path.clone());
                                } else {
                                    chosen = Some((picker.slot, entry.path.clone()));
                                }
                            }
                        }
                    });
            });
        if let Some(directory) = navigate {
            self.file_picker = Some(make_file_picker_state(picker.slot, directory));
        } else if let Some((slot, path)) = chosen {
            self.file_path_inputs[slot] = path.display().to_string();
            self.file_picker = None;
            self.load_file_slot(slot);
        } else if !opened {
            self.file_picker = None;
        }
    }

    pub(super) fn open_dropped_file(&mut self, slot: usize, path: PathBuf) {
        self.file_path_inputs[slot] = path.display().to_string();
        if slot == 1 && self.files.is_empty() {
            self.pending_browser_b = Some(path);
            self.file_error = Some("File B is queued. Load File A to open it.".into());
        } else {
            self.load_file_slot(slot);
        }
    }

    fn load_file_slot(&mut self, slot: usize) {
        if self.cell_edit_job.is_some()
            || self.pending_cell_edits.keys().any(|key| key.file == slot)
        {
            self.file_error = Some(format!(
                "File {} has pending or in-progress edits. Apply or discard them before replacing it.",
                if slot == 0 { "A" } else { "B" }
            ));
            return;
        }
        if self.plugins.is_syncing() {
            self.file_error =
                Some("Wait for synchronization to finish before opening files.".into());
            return;
        }
        let path = self.file_path_inputs[slot].trim();
        if path.is_empty() {
            self.file_error = Some(format!(
                "Enter a path for input file {}.",
                if slot == 0 { "A" } else { "B" }
            ));
            return;
        }
        if slot == 1 && self.files.is_empty() {
            self.file_error = Some("Load input file A before input file B.".into());
            return;
        }
        let path = PathBuf::from(path);
        let worker_path = path.clone();
        let options = OpenOptions {
            max_depth: self.settings.tree_max_depth,
            max_objects: self.settings.tree_max_objects,
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result =
                File::open_with_options(&worker_path, options).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        self.file_open_jobs[slot] = Some(receiver);
        self.file_open_paths[slot] = Some(path);
        self.file_error = None;
    }

    fn poll_file_open_jobs(&mut self) {
        let mut loaded_a = false;
        for slot in 0..2 {
            let result = self.file_open_jobs[slot]
                .as_ref()
                .map(|receiver| receiver.try_recv());
            match result {
                Some(Ok(Ok(file))) => {
                    loaded_a |= slot == 0;
                    self.file_open_jobs[slot] = None;
                    let path = self.file_open_paths[slot].take().unwrap();
                    if slot < self.files.len() {
                        self.files[slot] = file;
                        self.file_paths[slot] = path;
                    } else {
                        self.files.push(file);
                        self.file_paths.push(path);
                    }
                    self.file_path_inputs[slot] = self.file_paths[slot].display().to_string();
                    self.plugins.invalidate();
                    self.file_error = None;
                    self.reset_file_selection();
                }
                Some(Ok(Err(error))) => {
                    self.file_open_jobs[slot] = None;
                    self.file_open_paths[slot] = None;
                    self.file_error = Some(error);
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.file_open_jobs[slot] = None;
                    self.file_open_paths[slot] = None;
                    self.file_error = Some("File loading worker stopped unexpectedly.".into());
                }
                Some(Err(mpsc::TryRecvError::Empty)) | None => {}
            }
        }
        if loaded_a {
            if let Some(path) = self.pending_browser_b.take() {
                self.file_path_inputs[1] = path.display().to_string();
                self.load_file_slot(1);
            }
        }
        if !self.files.is_empty()
            && self.pending_browser_b.is_none()
            && self.file_open_jobs.iter().all(Option::is_none)
        {
            if let Some(selection) = self.sync_restore_selection.take() {
                if self
                    .files
                    .get(selection.file)
                    .is_some_and(|file| find_entry(&file.tree, &selection.path).is_some())
                {
                    self.selection = Some(selection);
                    self.refresh_selection();
                }
            }
        } else if self.file_error.is_some() && self.file_open_jobs.iter().all(Option::is_none) {
            self.sync_restore_selection = None;
        }
    }

    pub(crate) fn reset_file_selection(&mut self) {
        self.selection = None;
        self.selection_key = None;
        self.details_key = None;
        self.clear_selection_details();
        self.tree_geolocation_cache.clear();
        self.axis_names.clear();
        self.clear_selected_dataset_views();
        self.heatmap_overview_cache = None;
        self.heatmap_overview_job = None;
        self.heatmap_range_cache = None;
        self.heatmap_tick_view = None;
        self.heatmap_range_job = None;
        self.heatmap_colormap = None;
        self.draft_volume = None;
        self.applied_volume = None;
        self.map_controls = GeoViewControls::new(self.settings.map_show_pics)
            .with_dummy_rgb(self.settings.palette.accent);
        self.globe_controls = GeoViewControls::new(self.settings.map_show_pics)
            .with_dummy_rgb(self.settings.palette.accent);
        self.map_viewport = MapViewport::default();
        self.globe_auto_center_source = None;
        self.globe_auto_center_pending = false;
        self.globe_rotation = None;
        self.draft_slice = None;
        self.applied_slice = None;
        self.draft_table = None;
        self.applied_table = None;
        self.draft_profile = None;
        self.applied_profile = None;
    }
}

fn draw_file_picker_refresh_icon(ui: &Ui, palette: &Palette) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let radius = (max[0] - min[0]).min(max[1] - min[1]) * 0.31;
    // A clockwise, nearly complete ring with its arrowhead at the upper right.
    let start = 0.8_f32;
    let end = 5.95_f32;
    let point = |angle: f32| {
        [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]
    };
    let points = (0..=24)
        .map(|index| point(start + (end - start) * index as f32 / 24.0))
        .collect();
    let draw = ui.get_window_draw_list();
    draw.add_polyline(points, palette.text)
        .thickness(1.9)
        .build();
    let tip = point(end);
    let tangent = [-end.sin(), end.cos()];
    let back = [tip[0] - tangent[0] * 5.0, tip[1] - tangent[1] * 5.0];
    let side = [-tangent[1] * 3.2, tangent[0] * 3.2];
    for wing in [
        [back[0] + side[0], back[1] + side[1]],
        [back[0] - side[0], back[1] - side[1]],
    ] {
        draw.add_line(wing, tip, palette.text)
            .thickness(1.9)
            .build();
    }
}

fn draw_file_picker_icon(ui: &Ui, position: [f32; 2], is_directory: bool, palette: &Palette) {
    let draw = ui.get_window_draw_list();
    let [x, y] = position;
    if is_directory {
        draw.add_rect([x + 1.0, y + 6.0], [x + 15.0, y + 14.0], palette.accent)
            .filled(true)
            .rounding(2.0)
            .build();
        draw.add_rect([x + 2.0, y + 3.0], [x + 8.0, y + 8.0], palette.accent)
            .filled(true)
            .rounding(1.5)
            .build();
    } else {
        draw.add_rect([x + 4.0, y + 2.0], [x + 13.0, y + 15.0], palette.muted)
            .rounding(1.0)
            .thickness(1.2)
            .build();
        draw.add_line([x + 6.0, y + 7.0], [x + 11.0, y + 7.0], palette.muted)
            .thickness(1.0)
            .build();
        draw.add_line([x + 6.0, y + 10.0], [x + 11.0, y + 10.0], palette.muted)
            .thickness(1.0)
            .build();
    }
}
