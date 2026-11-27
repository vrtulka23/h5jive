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
        self.poll_bulk_replace_job();
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
        self.dataset_browser
            .poll(&self.settings.dataset_sources, &mut self.notices);
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
                    if file_path_field(ui,"input_a",&mut self.file_path_inputs[0],360.0) {
                        self.open_file_picker(0);
                    }
                    item_tip(ui, tutorial_tips, "Browse for file A", "Type a path or click the folder icon to browse for file A.");
                    if ui.button("Load A") {
                        self.load_file_slot(0);
                    }
                    item_tip(ui, tutorial_tips, "", "File A is the primary file for browsing and comparisons.");
                    ui.separator();
                    ui.text("Input file B");
                    if file_path_field(ui,"input_b",&mut self.file_path_inputs[1],360.0) {
                        self.open_file_picker(1);
                    }
                    item_tip(ui, tutorial_tips, "Browse for file B", "Type a path or click the folder icon to browse for file B.");
                    if ui.button("Load B") {
                        self.load_file_slot(1);
                    }
                    item_tip(ui, tutorial_tips, "", "Load A first, then B for comparisons.");
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
                    if ui.menu_item("Register datasets in DIPL...") {
                        self.show_dataset_registration_help = true;
                    }
                    item_tip(ui, tutorial_tips, "", "See how to register a Dataset Browser source and format.");
                    ui.separator();
                    if ui.menu_item_enabled_selected_no_shortcut(
                        "Tutorial tips",
                        self.settings.tutorial_tips,
                        true,
                    ) {
                        if let Err(error) = self
                            .settings
                            .set_tutorial_tips(!self.settings.tutorial_tips)
                        {
                            self.show_notice(
                                NoticeKind::Error,
                                format!("Could not save Tutorial tips: {error}"),
                            );
                        }
                    }
                    item_tip(ui, tutorial_tips, "Show extra guidance when hovering controls. This choice is saved in DIPL overrides.", "Uncheck this to keep only normal tooltips.");
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
        let panel_height = (size[1] - menu_height).max(1.0);
        if self.workspace_mode == WorkspaceMode::DatasetBrowser {
            self.view_rect = [0.0; 4];
            self.draw_dataset_browser(ui, menu_height, panel_height, flags);
            self.draw_help_dialogs(ui);
            self.draw_file_picker(ui);
            self.draw_global_status_overlays(ui, size);
            self.draw_file_drop_overlay(ui, size);
            return;
        }
        if self.workspace_mode != WorkspaceMode::DataInspector {
            self.view_rect = [0.0; 4];
            self.draw_workspace_placeholder(ui, plot_ui, menu_height, panel_height, flags);
            self.draw_help_dialogs(ui);
            self.draw_file_picker(ui);
            self.draw_global_status_overlays(ui, size);
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
                let mut picker_slot = None;
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
                                            if let Some(file) = self.files.get(index) {
                                                if draw_file_root(
                                                    ui,
                                                    file,
                                                    index,
                                                    &mut self.selection,
                                                    self.settings.tutorial_tips,
                                                    &self.settings.palette,
                                                ) {
                                                    picker_slot = Some(index);
                                                }
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
                                                if ui.selectable_config(format!("Choose a file for {label}...##empty_file_{index}")).build() {
                                                    picker_slot = Some(index);
                                                }
                                                item_tip(ui, self.settings.tutorial_tips,
                                                    "Choose a file",
                                                    "Open the file picker for this tab.");
                                            }
                                        });
                                }
                            }
                            self.startup_tab = None;
                        }
                    });
                if let Some(slot) = picker_slot {
                    self.open_file_picker(slot);
                }
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
        self.draw_global_status_overlays(ui, size);
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

    fn global_loading_label(&self) -> Option<String> {
        let mut tasks: Vec<String> = Vec::new();
        for (slot, job) in self.file_open_jobs.iter().enumerate() {
            if job.is_some() {
                tasks.push(if slot == 0 { "file A" } else { "file B" }.to_owned());
            }
        }
        if let Some(label) = self.map_loading_label() {
            tasks.push(label.to_owned());
        }
        if let Some(label) = self.heatmap_tile_loading_label() {
            tasks.push(label.to_owned());
        }
        if let Some(label) = self.volume_loading_label() {
            tasks.push(label.to_owned());
        }
        if let Some(label) = self.volume_raster_loading_label() {
            tasks.push(label.to_owned());
        }
        if self.profile_job.is_some() {
            let has_overview = self.profile_cache.as_ref().is_some_and(|(_, result)| {
                result
                    .as_ref()
                    .is_ok_and(|data| data.sample_positions.is_some())
            });
            tasks.push(
                if has_overview {
                    "1D profile detail"
                } else {
                    "1D profile"
                }
                .to_owned(),
            );
        }
        for (active, label) in [
            (self.heatmap_overview_job.is_some(), "2D image"),
            (self.heatmap_range_job.is_some(), "2D range"),
            (self.statistics_job.is_busy(), "stats"),
            (self.distribution_job.is_busy(), "distribution"),
            (self.cell_edit_job.is_some(), "cell edit"),
        ] {
            if active {
                tasks.push(label.to_owned());
            }
        }
        if let Some(job) = &self.bulk_replace_job {
            let processed = job.progress.load(Ordering::Relaxed);
            tasks.push(format!("bulk replace ({processed} cells)"));
        }
        if self
            .find_replace_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.search_job.is_some())
        {
            tasks.push("find matches".to_owned());
        }
        tasks.extend(
            self.dataset_browser
                .active_task_labels()
                .into_iter()
                .map(str::to_owned),
        );
        tasks.extend(
            self.plugins
                .active_task_labels()
                .into_iter()
                .map(str::to_owned),
        );
        if tasks.is_empty() {
            return None;
        }
        let mut grouped: Vec<(String, usize)> = Vec::new();
        for task in tasks {
            if let Some((_, count)) = grouped
                .iter_mut()
                .find(|(label, _)| label.as_str() == task.as_str())
            {
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
        Some(label)
    }

    fn draw_global_status_overlays(&mut self, ui: &Ui, size: [f32; 2]) {
        let mut bubbles = Vec::new();
        if let Some(label) = self.global_loading_label() {
            bubbles.push(("loading".to_owned(), None, NoticeKind::Loading, label));
        }
        for notice in self.notices.iter() {
            bubbles.push((
                format!("notice-{}", notice.id()),
                Some(notice.id()),
                notice.kind,
                notice.message.clone(),
            ));
        }
        let max_width = (size[0] - 30.0).min(520.0).max(90.0);
        let mut top = 8.0;
        let mut dismissed = None;
        for (id, notice_id, kind, message) in bubbles {
            let (height, close_clicked) = draw_notice_bubble(
                ui,
                &id,
                size[0] * 0.5,
                top,
                &message,
                kind,
                &self.settings.palette,
                max_width,
            );
            if close_clicked {
                dismissed = notice_id;
            }
            top += height + PANEL_GAP;
        }
        if let Some(id) = dismissed {
            self.dismiss_notice(id);
        }
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
            return Some("3D projection");
        };
        Some(
            match volume_key_cells(working).cmp(&volume_key_cells(previous)) {
                std::cmp::Ordering::Less => "3D projection (refining resolution)",
                std::cmp::Ordering::Greater => "3D projection (reducing detail)",
                std::cmp::Ordering::Equal => "3D projection region",
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
                    && self.bulk_replace_job.is_none()
                    && !self.plugins.is_syncing()
                    && self.file_open_jobs.iter().all(Option::is_none)
                    && self.pending_cell_edits.is_empty();
                let write_allowed = [can_write_files; 2];
                let mut picker_request = None;
                if let Some(selection) = self.plugins.draw(
                    ui,
                    plot_ui,
                    &mut self.files,
                    &self.file_paths,
                    &self.settings,
                    self.selection.as_ref(),
                    write_allowed,
                    &self.dataset_browser,
                    &mut picker_request,
                    &mut self.notices,
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
                if let Some((target, path)) = picker_request {
                    self.open_file_picker_for(target, path);
                }
                if self.plugins.has_sync_request() {
                    if !self.pending_cell_edits.is_empty()
                        || self.cell_edit_job.is_some()
                        || self.bulk_replace_job.is_some()
                    {
                        self.plugins.invalidate();
                        self.show_notice(
                            NoticeKind::Warning,
                            "Apply or discard pending cell edits before synchronizing files.",
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
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "Close##section_help",
                [0.0, 0.0],
            ) {
                ui.close_current_popup();
            }
        }
        self.draw_dataset_registration_help(ui);
        self.draw_about_dialog(ui);
    }

    fn draw_dataset_registration_help(&mut self, ui: &Ui) {
        const POPUP: &str = "Register datasets in DIPL###dataset_registration_help";
        if self.show_dataset_registration_help {
            ui.open_popup(POPUP);
            self.show_dataset_registration_help = false;
        }
        let mut open = true;
        if let Some(_popup) = ui.begin_modal_popup_config(POPUP).opened(&mut open).begin() {
            ui.set_window_size_with_cond([680.0, 485.0], Condition::FirstUseEver);
            ui.child_window("dataset_registration_help_content")
                .size([0.0, 0.0])
                .build(ui, || {
                    ui.text("Register a Dataset Browser source");
                    ui.separator();
                    ui.text_wrapped("1. Add a DIPL file under settings/dataset/sources/ (settings/runtime/dataset/sources/ in a source checkout). Give dataset_sources a unique ID, a local root directory, and a link to a dataset_formats entry.");
                    ui.spacing();
                    ui.text("dataset_sources[my_data] : dataset_source\n  name = \"My data\"\n  root = \"/data/my_data\"\n  format link = {?dataset_formats[era5_single_levels]}\n  tags = \"example, local\"\n  product_regex = \"^(?P<year>[0-9]{4})_(?P<month>[0-9]{2})[.]nc$\"");
                    ui.spacing();
                    ui.text_wrapped("2. The regex matches file paths relative to root. Named year, month, day, and orbit captures populate the Browser columns. Use an existing format when its coordinate rules fit your products.");
                    ui.spacing();
                    ui.text_wrapped("3. Register the new source file in settings/DIPfile under code[]. For a new format, also add dataset/formats/<id>.dip using the dataset_format schema, register that file under code[], and point the source's format link to dataset_formats[<id>]. A format can define coordinate groups, latitude/longitude and bounds paths, a time coordinate name, and scan direction flags.");
                    ui.spacing();
                    ui.text_wrapped("4. Restart H5Jive, select the source in Dataset Browser, and click Refresh index. Use h5jive --settings-help 'dataset_sources[my_data].product_regex' for schema guidance. For an existing source, change only its root in settings/overrides.dip; that file is preserved by the installer.");
                });
        };
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

fn make_file_picker_state(
    target: FilePickerTarget,
    directory: PathBuf,
    filename: String,
) -> FilePickerState {
    let mut state = FilePickerState {
        target,
        needs_open: true,
        directory: directory.clone(),
        filename,
        entries: Vec::new(),
        selected: None,
        pending_delete: None,
        error: None,
        creating_folder: false,
        new_folder_name: String::new(),
    };
    match std::fs::read_dir(&directory) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        let is_directory = path.is_dir();
                        if !is_directory && (!path.is_file() || !target.accepts(&path)) {
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

fn create_file_picker_folder(directory: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err("Enter a folder name without path separators".into());
    }
    let path = directory.join(name);
    std::fs::create_dir(&path).map_err(|error| format!("Could not create folder: {error}"))?;
    Ok(path)
}

fn delete_file_picker_selection(
    picker: &FilePickerState,
    path: &std::path::Path,
    loaded: &[PathBuf],
    loading: &[Option<PathBuf>; 2],
) -> Result<(), String> {
    if picker.pending_delete.as_deref() != Some(path)
        || picker.selected.as_deref() != Some(path)
        || !picker
            .entries
            .iter()
            .any(|entry| entry.path == path && !entry.is_directory)
    {
        return Err("Select a file in the current folder before deleting it".into());
    }
    if loaded.iter().any(|current| same_file(current, path))
        || loading
            .iter()
            .flatten()
            .any(|current| same_file(current, path))
    {
        return Err("Close this file in H5Jive before deleting it".into());
    }
    if !path.is_file() {
        return Err("The selected file is no longer available; refresh the folder".into());
    }
    std::fs::remove_file(path).map_err(|error| format!("Could not delete file: {error}"))
}

pub(super) fn is_supported_input_file(path: &std::path::Path) -> bool {
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
    use super::{
        create_file_picker_folder, delete_file_picker_selection, is_supported_input_file,
        make_file_picker_state, FilePickerTarget,
    };
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

    #[test]
    fn shared_picker_uses_the_destination_file_type() {
        assert!(FilePickerTarget::ViewExport.accepts(Path::new("figure.JPEG")));
        assert!(!FilePickerTarget::ViewExport.accepts(Path::new("figure.nc")));
        assert!(FilePickerTarget::ManipulationOutput.accepts(Path::new("derived.hdf5")));
        assert!(!FilePickerTarget::ManipulationOutput.accepts(Path::new("derived.png")));
        assert!(FilePickerTarget::SubsetOutput.accepts(Path::new("subset.nc")));
        assert!(!FilePickerTarget::SubsetOutput.accepts(Path::new("subset.h5")));
        let folder = std::env::temp_dir();
        let picker =
            make_file_picker_state(FilePickerTarget::SubsetOutput, folder, "new.nc".into());
        assert_eq!(picker.filename, "new.nc");
        assert!(picker.target.is_save());
    }

    #[test]
    fn new_folder_is_created_and_visible_to_shared_picker() {
        let root = std::env::temp_dir().join(format!(
            "h5jive-picker-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        assert!(create_file_picker_folder(&root, "../bad").is_err());
        let created = create_file_picker_folder(&root, " New folder ").unwrap();
        assert_eq!(created.file_name().unwrap(), "New folder");
        assert!(created.is_dir());
        assert!(create_file_picker_folder(&root, "New folder").is_err());
        let picker =
            make_file_picker_state(FilePickerTarget::SubsetOutput, root.clone(), String::new());
        assert!(picker
            .entries
            .iter()
            .any(|entry| entry.path == created && entry.is_directory));
        std::fs::remove_dir(created).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn selected_file_requires_confirmation_and_cannot_be_open_in_h5jive() {
        let root = std::env::temp_dir().join(format!(
            "h5jive-delete-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("sample.nc");
        std::fs::write(&file, b"temporary test file").unwrap();
        let mut picker =
            make_file_picker_state(FilePickerTarget::SubsetOutput, root.clone(), String::new());
        picker.selected = Some(file.clone());
        assert!(delete_file_picker_selection(&picker, &file, &[], &[None, None]).is_err());
        assert!(file.exists());
        picker.pending_delete = Some(file.clone());
        assert!(
            delete_file_picker_selection(&picker, &file, &[file.clone()], &[None, None]).is_err()
        );
        assert!(file.exists());
        delete_file_picker_selection(&picker, &file, &[], &[None, None]).unwrap();
        assert!(!file.exists());
        std::fs::remove_dir(root).unwrap();
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
        self.open_file_picker_for(
            FilePickerTarget::Input(slot),
            self.file_path_inputs[slot].clone(),
        );
    }

    pub(super) fn open_file_picker_for(&mut self, target: FilePickerTarget, current: String) {
        let current = PathBuf::from(current.trim());
        let directory = if current.is_dir() {
            current.clone()
        } else {
            current
                .parent()
                .filter(|parent| parent.is_dir())
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("."))
        };
        let filename = if current.is_dir() {
            String::new()
        } else {
            current
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        self.file_picker = Some(make_file_picker_state(target, directory, filename));
    }

    fn draw_file_picker(&mut self, ui: &Ui) {
        const POPUP: &str = "File picker###file_picker";
        let Some(picker) = self.file_picker.as_mut() else {
            return;
        };
        if picker.needs_open {
            ui.open_popup(POPUP);
            picker.needs_open = false;
        }
        let mut opened = true;
        let mut navigate = None;
        let mut chosen = None;
        if let Some(_popup) = ui
            .begin_modal_popup_config(POPUP)
            .opened(&mut opened)
            .begin()
        {
            ui.set_window_size_with_cond([640.0, 520.0], Condition::FirstUseEver);
            ui.text(picker.target.title());
            ui.separator();
            let icon_button_size = ui.frame_height() + 6.0;
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "##file_picker_refresh",
                [icon_button_size; 2],
            ) {
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
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "##file_picker_new_folder",
                [icon_button_size; 2],
            ) {
                picker.creating_folder = !picker.creating_folder;
                picker.error = None;
            }
            draw_file_picker_new_folder_icon(ui, &self.settings.palette);
            item_tip(
                ui,
                self.settings.tutorial_tips,
                "Create a folder",
                "Create a new folder here and open it in the picker.",
            );
            let selected_file = picker.selected.as_ref().filter(|path| {
                picker
                    .entries
                    .iter()
                    .any(|entry| &entry.path == *path && !entry.is_directory)
            });
            ui.same_line_with_spacing(0.0, PANEL_GAP);
            let _delete_disabled = ui.begin_disabled_with_cond(selected_file.is_none());
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "##file_picker_delete",
                [icon_button_size; 2],
            ) {
                picker.pending_delete = selected_file.cloned();
                picker.error = None;
            }
            draw_file_picker_delete_icon(ui, &self.settings.palette, selected_file.is_some());
            drop(_delete_disabled);
            item_tip(
                ui,
                self.settings.tutorial_tips,
                "Delete the selected file",
                "Select a file in the list, then confirm its deletion. Folders cannot be deleted here.",
            );
            let path_text = picker.directory.display().to_string();
            let path_width = ui.calc_text_size(&path_text)[0];
            let delete_right = ui.item_rect_max()[0];
            let content_right = ui.cursor_screen_pos()[0] + ui.content_region_avail()[0];
            if path_width <= content_right - delete_right - PANEL_GAP {
                ui.same_line_with_spacing(0.0, PANEL_GAP);
                ui.align_text_to_frame_padding();
                ui.text_wrapped(&path_text);
            } else {
                ui.text_wrapped(&path_text);
            }
            if picker.creating_folder {
                ui.set_next_item_width((ui.content_region_avail()[0] - 150.0).max(120.0));
                ui.input_text("##file_picker_folder_name", &mut picker.new_folder_name)
                    .build();
                ui.same_line();
                if draw_accent_button(
                    ui,
                    &self.settings.palette,
                    "Create##file_picker_folder",
                    [0.0, 0.0],
                ) {
                    match create_file_picker_folder(&picker.directory, &picker.new_folder_name) {
                        Ok(path) => navigate = Some(path),
                        Err(error) => picker.error = Some(error),
                    }
                }
                ui.same_line();
                if draw_accent_button(
                    ui,
                    &self.settings.palette,
                    "Cancel##file_picker_folder",
                    [0.0, 0.0],
                ) {
                    picker.creating_folder = false;
                    picker.new_folder_name.clear();
                    picker.error = None;
                }
            }
            if let Some(path) = picker.pending_delete.clone() {
                ui.separator();
                ui.text_wrapped(format!("Permanently delete {}?", path.display()));
                if draw_accent_button(
                    ui,
                    &self.settings.palette,
                    "Delete file##file_picker_confirm_delete",
                    [0.0, 0.0],
                ) {
                    match delete_file_picker_selection(
                        picker,
                        &path,
                        &self.file_paths,
                        &self.file_open_paths,
                    ) {
                        Ok(()) => {
                            if path
                                .file_name()
                                .is_some_and(|name| name == picker.filename.as_str())
                            {
                                picker.filename.clear();
                            }
                            navigate = Some(picker.directory.clone());
                        }
                        Err(error) => picker.error = Some(error),
                    }
                    picker.pending_delete = None;
                }
                ui.same_line();
                if draw_accent_button(
                    ui,
                    &self.settings.palette,
                    "Keep file##file_picker_cancel_delete",
                    [0.0, 0.0],
                ) {
                    picker.pending_delete = None;
                }
            }
            if let Some(error) = &picker.error {
                ui.text_wrapped(error);
            }
            ui.separator();
            ui.child_window("##file_picker_entries")
                .size([
                    0.0,
                    if picker.target.is_save() {
                        -120.0
                    } else {
                        -42.0
                    },
                ])
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
                            picker.pending_delete = None;
                            picker.error = None;
                        }
                        if hovered && ui.is_mouse_double_clicked(dear_app::imgui::MouseButton::Left)
                        {
                            if entry.is_directory {
                                navigate = Some(entry.path.clone());
                            } else {
                                if picker.target.is_save() {
                                    picker.filename = entry.label.clone();
                                } else {
                                    chosen = Some((picker.target, entry.path.clone()));
                                    ui.close_current_popup();
                                }
                            }
                        }
                    }
                });
            if picker.target.is_save() {
                ui.separator();
                ui.text("File name");
                ui.set_next_item_width(ui.content_region_avail()[0].max(120.0));
                ui.input_text("##picker_save_name", &mut picker.filename)
                    .build();
                if draw_accent_button(ui, &self.settings.palette, "Use path", [0.0, 0.0]) {
                    let name = picker.filename.trim();
                    if name.is_empty()
                        || name == "."
                        || name == ".."
                        || name.contains('/')
                        || name.contains('\\')
                    {
                        picker.error = Some("Enter a filename without folder separators".into());
                    } else {
                        let path = picker.directory.join(name);
                        if picker.target.accepts(&path) {
                            chosen = Some((picker.target, path));
                            ui.close_current_popup();
                        } else {
                            picker.error =
                                Some("Choose a filename with the expected extension".into());
                        }
                    }
                }
            }
        } else if !ui.is_popup_open(POPUP) {
            picker.needs_open = true;
        }
        if let Some(directory) = navigate {
            let mut next =
                make_file_picker_state(picker.target, directory, picker.filename.clone());
            next.needs_open = false;
            self.file_picker = Some(next);
        } else if let Some((target, path)) = chosen {
            self.file_picker = None;
            match target {
                FilePickerTarget::Input(slot) => {
                    self.file_path_inputs[slot] = path.display().to_string();
                    self.load_file_slot(slot);
                }
                FilePickerTarget::ViewExport => {
                    if let Some(dialog) = self.export_dialog.as_mut() {
                        dialog.path = path.display().to_string();
                    }
                }
                FilePickerTarget::ManipulationOutput => self
                    .plugins
                    .set_picker_path(target, path.display().to_string()),
                FilePickerTarget::SubsetOutput => self
                    .plugins
                    .set_picker_path(target, path.display().to_string()),
            }
        } else if !opened {
            self.file_picker = None;
        }
    }

    pub(super) fn open_dropped_file(&mut self, slot: usize, path: PathBuf) {
        self.file_path_inputs[slot] = path.display().to_string();
        if slot == 1 && self.files.is_empty() {
            self.pending_browser_b = Some(path);
            self.show_notice(
                NoticeKind::Warning,
                "File B is queued. Load File A to open it.",
            );
        } else {
            self.load_file_slot(slot);
        }
    }

    fn load_file_slot(&mut self, slot: usize) {
        if self.cell_edit_job.is_some()
            || self.bulk_replace_job.is_some()
            || self.pending_cell_edits.keys().any(|key| key.file == slot)
        {
            self.show_notice(
                NoticeKind::Warning,
                format!(
                    "File {} has pending or in-progress edits. Apply or discard them before replacing it.",
                    if slot == 0 { "A" } else { "B" }
                ),
            );
            return;
        }
        if self.plugins.is_syncing() {
            self.show_notice(
                NoticeKind::Warning,
                "Wait for synchronization to finish before opening files.",
            );
            return;
        }
        let path = self.file_path_inputs[slot].trim();
        if path.is_empty() {
            self.show_notice(
                NoticeKind::Warning,
                format!(
                    "Enter a path for input file {}.",
                    if slot == 0 { "A" } else { "B" }
                ),
            );
            return;
        }
        if slot == 1 && self.files.is_empty() {
            self.show_notice(
                NoticeKind::Warning,
                "Load input file A before input file B.",
            );
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
                    let different_file = self.file_paths.get(slot) != Some(&path);
                    if slot < self.files.len() {
                        self.files[slot] = file;
                        self.file_paths[slot] = path;
                    } else {
                        self.files.push(file);
                        self.file_paths.push(path);
                    }
                    self.file_path_inputs[slot] = self.file_paths[slot].display().to_string();
                    self.plugins.invalidate();
                    self.reset_file_selection();
                    if different_file {
                        self.switch_view(ViewMode::Data);
                    }
                }
                Some(Ok(Err(error))) => {
                    self.file_open_jobs[slot] = None;
                    self.file_open_paths[slot] = None;
                    self.sync_restore_selection = None;
                    self.show_notice(NoticeKind::Error, error);
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.file_open_jobs[slot] = None;
                    self.file_open_paths[slot] = None;
                    self.sync_restore_selection = None;
                    self.show_notice(
                        NoticeKind::Error,
                        "File loading worker stopped unexpectedly.",
                    );
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

fn draw_file_picker_new_folder_icon(ui: &Ui, palette: &Palette) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let draw = ui.get_window_draw_list();
    draw.add_rect(
        [center[0] - 8.0, center[1] - 3.0],
        [center[0] + 8.0, center[1] + 6.0],
        palette.text,
    )
    .rounding(2.0)
    .thickness(1.5)
    .build();
    draw.add_line(
        [center[0] - 7.0, center[1] - 4.0],
        [center[0] - 1.0, center[1] - 4.0],
        palette.text,
    )
    .thickness(1.5)
    .build();
    draw.add_line(
        [center[0] + 4.0, center[1] - 7.0],
        [center[0] + 4.0, center[1] + 1.0],
        palette.text,
    )
    .thickness(1.7)
    .build();
    draw.add_line(
        [center[0], center[1] - 3.0],
        [center[0] + 8.0, center[1] - 3.0],
        palette.text,
    )
    .thickness(1.7)
    .build();
}

fn draw_file_picker_delete_icon(ui: &Ui, palette: &Palette, enabled: bool) {
    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let color = if enabled { palette.text } else { palette.muted };
    let draw = ui.get_window_draw_list();
    draw.add_line(
        [center[0] - 7.0, center[1] - 6.0],
        [center[0] + 7.0, center[1] - 6.0],
        color,
    )
    .thickness(1.7)
    .build();
    draw.add_line(
        [center[0] - 2.5, center[1] - 6.0],
        [center[0] - 2.5, center[1] - 8.0],
        color,
    )
    .thickness(1.7)
    .build();
    draw.add_line(
        [center[0] + 2.5, center[1] - 6.0],
        [center[0] + 2.5, center[1] - 8.0],
        color,
    )
    .thickness(1.7)
    .build();
    draw.add_rect(
        [center[0] - 6.0, center[1] - 5.0],
        [center[0] + 6.0, center[1] + 6.0],
        color,
    )
    .rounding(1.5)
    .thickness(1.7)
    .build();
    draw.add_line(
        [center[0] - 3.0, center[1] - 2.0],
        [center[0] - 3.0, center[1] + 3.0],
        color,
    )
    .thickness(1.5)
    .build();
    draw.add_line(
        [center[0], center[1] - 2.0],
        [center[0], center[1] + 3.0],
        color,
    )
    .thickness(1.5)
    .build();
    draw.add_line(
        [center[0] + 3.0, center[1] - 2.0],
        [center[0] + 3.0, center[1] + 3.0],
        color,
    )
    .thickness(1.5)
    .build();
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
