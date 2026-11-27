use super::super::*;

const POPUP: &str = "Find / Replace##data_view";
const PAGE: usize = 10;
const MATCH_MODES: &[&str] = &["Exact", "Tolerance", "Round to decimals"];
const DIALOG_PADDING: [f32; 2] = [12.0, 10.0];

fn invalidate_search(dialog: &mut FindReplaceDialog) {
    dialog.cancel.store(true, Ordering::Relaxed);
    dialog.search_job = None;
    dialog.summary = None;
    dialog.first_result = 0;
}

fn resolve_numeric_match(dialog: &FindReplaceDialog) -> Result<h5jive_data::NumericMatch, String> {
    match dialog.match_mode {
        0 => Ok(h5jive_data::NumericMatch::Exact),
        1 => {
            let tolerance = dialog
                .tolerance
                .trim()
                .parse::<f64>()
                .map_err(|_| "Enter a numeric tolerance for matching.".to_string())?;
            if !tolerance.is_finite() || tolerance < 0.0 {
                return Err("Tolerance must be a non-negative finite number.".into());
            }
            Ok(h5jive_data::NumericMatch::Tolerance(tolerance))
        }
        _ => {
            let decimals = dialog.decimals.clamp(0, 15) as u32;
            Ok(h5jive_data::NumericMatch::Rounded(decimals))
        }
    }
}

impl App {
    pub(super) fn draw_find_replace_action(
        &mut self,
        ui: &Ui,
        selection: &Selection,
        _shape: &[usize],
    ) {
        let width = ui.content_region_avail()[0].max(1.0);
        if ui.button_with_size("Find / Replace", [width, ui.frame_height()]) {
            if self
                .find_replace_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.selection == *selection)
            {
                let dialog = self.find_replace_dialog.as_mut().unwrap();
                dialog.open_requested = true;
                dialog.preserved = false;
                dialog.message = None;
            } else {
                self.find_replace_dialog = Some(FindReplaceDialog {
                    selection: selection.clone(),
                    open_requested: true,
                    needle: String::new(),
                    replacement: String::new(),
                    match_mode: 0,
                    tolerance: String::new(),
                    decimals: 2,
                    first_result: 0,
                    summary: None,
                    progress: Arc::new(AtomicU64::new(0)),
                    cancel: Arc::new(AtomicBool::new(false)),
                    search_job: None,
                    message: None,
                    preserved: false,
                });
            }
        }
        item_tip(
            ui,
            self.settings.tutorial_tips,
            "",
            "Search every cell in the selected dataset, including pinned dimensions.",
        );
    }

    fn start_numeric_search(&mut self) {
        let Some(dialog) = self.find_replace_dialog.as_mut() else {
            return;
        };
        if dialog.needle.trim().is_empty() {
            dialog.message = Some("Enter a numeric value to find.".into());
            return;
        }
        let rule = match resolve_numeric_match(dialog) {
            Ok(rule) => rule,
            Err(message) => {
                dialog.message = Some(message);
                return;
            }
        };
        dialog.cancel.store(true, Ordering::Relaxed);
        dialog.cancel = Arc::new(AtomicBool::new(false));
        dialog.progress = Arc::new(AtomicU64::new(0));
        dialog.message = None;
        let path = self.file_paths[dialog.selection.file].clone();
        let dataset = dialog.selection.path.clone();
        let options = OpenOptions {
            max_depth: self.settings.tree_max_depth,
            max_objects: self.settings.tree_max_objects,
        };
        let needle = dialog.needle.clone();
        let first = dialog.first_result;
        let progress = Arc::clone(&dialog.progress);
        let cancel = Arc::clone(&dialog.cancel);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = File::open_with_options(&path, options)
                .and_then(|file| {
                    file.search_numeric(&dataset, &needle, rule, first, PAGE, &progress, &cancel)
                })
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        dialog.search_job = Some(receiver);
    }

    pub(super) fn draw_find_replace_dialog(
        &mut self,
        ui: &Ui,
        selection: &Selection,
        _shape: &[usize],
    ) {
        let Some(dialog) = self.find_replace_dialog.as_mut() else {
            return;
        };
        if dialog.selection != *selection {
            dialog.cancel.store(true, Ordering::Relaxed);
            self.find_replace_dialog = None;
            return;
        }
        let mut search_error = None;
        if let Some(receiver) = &dialog.search_job {
            match receiver.try_recv() {
                Ok(result) => {
                    if let Err(error) = &result {
                        search_error = Some(format!("Search failed: {error}"));
                    }
                    dialog.summary = Some(result);
                    dialog.search_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    search_error = Some("Search stopped unexpectedly.".into());
                    dialog.search_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(message) = search_error {
            self.notices.error(message);
        }
        if dialog.open_requested {
            ui.open_popup(POPUP);
            dialog.open_requested = false;
        }
        let mut open = true;
        let mut start_search = false;
        let mut start_replace = false;
        let mut navigate = None;
        let _window_padding = ui.push_style_var(StyleVar::WindowPadding(DIALOG_PADDING));
        if let Some(_popup) = ui.begin_modal_popup_config(POPUP).opened(&mut open).begin() {
            ui.set_window_size_with_cond([620.0, 400.0], Condition::FirstUseEver);
            let palette = self.settings.palette.clone();

            let width = ui.content_region_avail()[0];
            let controls_width = 250.0_f32;
            let matches_width = (width - controls_width - PANEL_GAP).max(220.0);
            if let Some(_table) = ui.begin_table("find_replace_layout", 2) {
                ui.table_setup_column_fixed_width(
                    "Controls",
                    TableColumnFlags::NONE,
                    controls_width,
                );
                ui.table_setup_column_fixed_width("Matches", TableColumnFlags::NONE, matches_width);
                ui.table_next_row();
                ui.table_next_column();

                ui.text("Find value");
                ui.set_next_item_width(-1.0);
                if ui.input_text("##find_value", &mut dialog.needle).build() {
                    invalidate_search(dialog);
                }
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "",
                    "Enter a number, NaN, or Fill to search every cell.",
                );

                ui.text("Match");
                ui.set_next_item_width(-1.0);
                if ui.combo_simple_string("##match_mode", &mut dialog.match_mode, MATCH_MODES) {
                    invalidate_search(dialog);
                }
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "",
                    "Choose how closely cells must match the find value.",
                );

                if dialog.match_mode == 1 {
                    ui.text("Tolerance");
                    ui.set_next_item_width(-1.0);
                    if ui.input_text("##tolerance", &mut dialog.tolerance).build() {
                        invalidate_search(dialog);
                    }
                    item_tip(
                        ui,
                        self.settings.tutorial_tips,
                        "",
                        "Match values within this absolute distance of the find value.",
                    );
                } else if dialog.match_mode == 2 {
                    ui.text("Decimals");
                    ui.set_next_item_width(-1.0);
                    if ui.slider("##decimals", 0, 12, &mut dialog.decimals) {
                        invalidate_search(dialog);
                    }
                    item_tip(
                        ui,
                        self.settings.tutorial_tips,
                        "",
                        "Match values that round to the same number of decimals.",
                    );
                }

                ui.spacing();
                let searching = dialog.search_job.is_some();
                {
                    let _disabled = ui.begin_disabled_with_cond(searching);
                    if draw_accent_button(ui, &palette, "Find all", [0.0, 0.0]) {
                        start_search = true;
                    }
                }
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "",
                    "Scan the whole dataset and show the first matches.",
                );
                if searching {
                    ui.same_line_with_spacing(0.0, PANEL_GAP);
                    if draw_accent_button(ui, &palette, "Cancel search", [0.0, 0.0]) {
                        dialog.cancel.store(true, Ordering::Relaxed);
                    }
                    item_tip(
                        ui,
                        self.settings.tutorial_tips,
                        "",
                        "Stop the scan and keep any matches found so far.",
                    );
                }

                ui.separator();
                ui.text("Replace with");
                ui.set_next_item_width(-1.0);
                ui.input_text("##replace_with", &mut dialog.replacement)
                    .build();
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "",
                    "Value to write into every match, such as a number, NaN, or Fill.",
                );
                let staged = self
                    .pending_cell_edits
                    .keys()
                    .any(|key| key.file == selection.file);
                if staged {
                    ui.text_wrapped(
                        "Apply or discard staged edits in this file before replacing values.",
                    );
                }
                let can_replace_all = matches!(&dialog.summary, Some(Ok(summary)) if summary.matches > 0)
                    && !dialog.replacement.trim().is_empty()
                    && !staged
                    && dialog.search_job.is_none()
                    && self.bulk_replace_job.is_none();
                {
                    let _disabled = ui.begin_disabled_with_cond(!can_replace_all);
                    if draw_accent_button(ui, &palette, "Replace all matches", [0.0, 0.0]) {
                        start_replace = true;
                        ui.close_current_popup();
                    }
                }
                item_tip(
                    ui,
                    self.settings.tutorial_tips,
                    "Run Find all and enter a replacement to enable.",
                    "Write the replacement to every match and reload the file.",
                );
                if let Some(message) = &dialog.message {
                    ui.text_wrapped(message);
                }

                ui.table_next_column();
                let tutorial_tips = self.settings.tutorial_tips;
                ui.child_window("find_replace_matches")
                    .size([0.0, 0.0])
                    .border(true)
                    .child_flags(ChildFlags::ALWAYS_USE_WINDOW_PADDING)
                    .build(ui, || {
                        if let Some(summary) = &dialog.summary {
                            match summary {
                                Ok(summary) => {
                                    let total_pages = summary.matches.div_ceil(PAGE as u64).max(1);
                                    let current_page =
                                        (dialog.first_result / (PAGE as u64) + 1).min(total_pages);
                                    let has_previous = dialog.first_result > 0;
                                    let has_next =
                                        dialog.first_result + (PAGE as u64) < summary.matches;

                                    let button_size = ui.frame_height();
                                    let line_height = ui.text_line_height();
                                    let row_height = (2.0 * line_height).max(button_size);
                                    let row_min = ui.cursor_screen_pos();
                                    let row_width = ui.content_region_avail()[0];
                                    ui.set_cursor_screen_pos([
                                        row_min[0],
                                        row_min[1] + (row_height - button_size) * 0.5,
                                    ]);
                                    {
                                        let _disabled = ui.begin_disabled_with_cond(!has_previous);
                                        if draw_accent_button(
                                            ui,
                                            &palette,
                                            "<",
                                            [button_size, button_size],
                                        ) {
                                            dialog.first_result =
                                                dialog.first_result.saturating_sub(PAGE as u64);
                                            start_search = true;
                                        }
                                    }
                                    item_tip(
                                        ui,
                                        tutorial_tips,
                                        "",
                                        "Show the previous page of matches.",
                                    );
                                    ui.set_cursor_screen_pos([
                                        row_min[0] + (row_width - button_size).max(0.0),
                                        row_min[1] + (row_height - button_size) * 0.5,
                                    ]);
                                    {
                                        let _disabled = ui.begin_disabled_with_cond(!has_next);
                                        if draw_accent_button(
                                            ui,
                                            &palette,
                                            ">",
                                            [button_size, button_size],
                                        ) {
                                            dialog.first_result += PAGE as u64;
                                            start_search = true;
                                        }
                                    }
                                    item_tip(
                                        ui,
                                        tutorial_tips,
                                        "",
                                        "Show the next page of matches.",
                                    );
                                    let matches_text = format!("{} matches", summary.matches);
                                    let page_text = format!("Page {current_page}/{total_pages}");
                                    let text_top =
                                        row_min[1] + (row_height - 2.0 * line_height) * 0.5;
                                    ui.set_cursor_screen_pos([
                                        row_min[0]
                                            + (row_width - ui.calc_text_size(&matches_text)[0])
                                                .max(0.0)
                                                * 0.5,
                                        text_top,
                                    ]);
                                    ui.text(&matches_text);
                                    ui.set_cursor_screen_pos([
                                        row_min[0]
                                            + (row_width - ui.calc_text_size(&page_text)[0])
                                                .max(0.0)
                                                * 0.5,
                                        text_top + line_height,
                                    ]);
                                    ui.text(&page_text);
                                    ui.set_cursor_screen_pos([row_min[0], row_min[1] + row_height]);
                                    ui.spacing();
                                    for found in &summary.samples {
                                        let label =
                                            format!("{:?} = {}", found.coordinates, found.value);
                                        if ui.selectable(label) {
                                            navigate = Some(found.coordinates.clone());
                                            ui.close_current_popup();
                                        }
                                        item_tip(
                                            ui,
                                            tutorial_tips,
                                            "",
                                            "Click a match to pin the table to its cell.",
                                        );
                                    }
                                }
                                Err(_) => ui.text_wrapped(
                                    "Search failed. See the error notice for details.",
                                ),
                            }
                        } else if dialog.search_job.is_none() {
                            ui.text_wrapped("Run Find all to list matches here.");
                        }
                    });
            }
        }
        if !open {
            if self
                .find_replace_dialog
                .as_ref()
                .is_some_and(|dialog| !dialog.preserved)
            {
                if let Some(dialog) = self.find_replace_dialog.take() {
                    dialog.cancel.store(true, Ordering::Relaxed);
                }
            }
            return;
        }
        if start_search {
            self.start_numeric_search();
        }
        if let Some(coordinates) = navigate {
            if let Some(slice) = self.applied_table.as_mut() {
                slice.pinned.clone_from(&coordinates);
                self.draft_table = Some(slice.clone());
                self.table_column_page =
                    coordinates[slice.column_axis] / self.settings.table_columns_per_page;
                self.table_scroll_to_row = Some(slice.row_axis.map_or(0, |axis| coordinates[axis]));
                self.region_cache.clear();
            }
            if let Some(dialog) = self.find_replace_dialog.as_mut() {
                dialog.preserved = true;
            }
        }
        if start_replace {
            self.start_bulk_replace(selection);
        }
        if !ui.is_popup_open(POPUP)
            && self.bulk_replace_job.is_none()
            && self
                .find_replace_dialog
                .as_ref()
                .is_some_and(|dialog| !dialog.open_requested && !dialog.preserved)
        {
            if let Some(dialog) = self.find_replace_dialog.take() {
                dialog.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    fn start_bulk_replace(&mut self, selection: &Selection) {
        let Some(dialog) = self.find_replace_dialog.as_mut() else {
            return;
        };
        let Some(Ok(summary)) = &dialog.summary else {
            return;
        };
        let rule = match resolve_numeric_match(dialog) {
            Ok(rule) => rule,
            Err(message) => {
                dialog.message = Some(message);
                dialog.open_requested = true;
                return;
            }
        };
        if self.cell_edit_job.is_some()
            || self.bulk_replace_job.is_some()
            || self.plugins.is_busy()
            || self.file_open_jobs.iter().any(Option::is_some)
        {
            dialog.message = Some("Wait for file operations to finish.".into());
            dialog.open_requested = true;
            return;
        }
        let slot = selection.file;
        let path = self.file_paths[slot].clone();
        if self
            .file_paths
            .iter()
            .enumerate()
            .any(|(other, candidate)| other != slot && same_file(candidate, &path))
        {
            dialog.message =
                Some("The same file is open in both slots. Close the other copy first.".into());
            dialog.open_requested = true;
            return;
        }
        if let Err(error) =
            self.files[slot].validate_numeric_edit(&selection.path, &dialog.replacement)
        {
            dialog.message = Some(error.to_string());
            dialog.open_requested = true;
            return;
        }
        if let Err(error) = self.files[slot].release_for_edit() {
            dialog.message = Some(error.to_string());
            dialog.open_requested = true;
            return;
        }
        let dataset = selection.path.clone();
        let needle = dialog.needle.clone();
        let replacement = dialog.replacement.clone();
        let expected = summary.matches;
        let progress = Arc::new(AtomicU64::new(0));
        let worker_progress = Arc::clone(&progress);
        let options = OpenOptions {
            max_depth: self.settings.tree_max_depth,
            max_objects: self.settings.tree_max_objects,
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let outcome = File::replace_numeric_matches(
                &path,
                &dataset,
                &needle,
                &replacement,
                rule,
                expected,
                &worker_progress,
            );
            let reopened =
                File::open_with_options(&path, options).map_err(|error| error.to_string());
            let _ = sender.send((outcome, reopened));
        });
        self.bulk_replace_job = Some(BulkReplaceJob {
            selection: selection.clone(),
            progress,
            receiver,
        });
        self.find_replace_dialog = None;
        self.reset_file_selection();
    }

    pub(crate) fn poll_bulk_replace_job(&mut self) {
        let Some(job) = &self.bulk_replace_job else {
            return;
        };
        let result = job.receiver.try_recv();
        match result {
            Ok((outcome, reopened)) => {
                let job = self.bulk_replace_job.take().unwrap();
                let slot = job.selection.file;
                match reopened {
                    Ok(file) => {
                        self.files[slot] = file;
                        self.reset_file_selection();
                        self.selection = Some(job.selection);
                        self.refresh_selection();
                        self.region_cache.clear();
                        self.plugins.invalidate();
                    }
                    Err(error) => {
                        self.show_notice(
                            NoticeKind::Error,
                            format!("Could not reopen file after replacing values: {error}"),
                        );
                    }
                }
                match outcome.error {
                    Some(error) => {
                        self.show_notice(NoticeKind::Error, format!("Replace stopped: {error}"))
                    }
                    None => self.show_notice(
                        NoticeKind::Success,
                        format!("Replaced {} values.", outcome.replaced),
                    ),
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.bulk_replace_job = None;
                self.show_notice(NoticeKind::Error, "Replace worker stopped unexpectedly.");
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
}
