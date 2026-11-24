use super::super::*;

impl App {
    pub(super) fn draw_cell_edit_bar(&mut self, ui: &Ui, selection: &Selection) {
        let count = self
            .pending_cell_edits
            .keys()
            .filter(|key| key.file == selection.file && key.path == selection.path)
            .count();
        let elsewhere = self.pending_cell_edits.len() - count;
        if count == 0 && elsewhere == 0 && self.cell_edit_message.is_none() {
            return;
        }
        if count > 0 {
            let status = format!(
                "{count} pending {} in File {}",
                if count == 1 { "change" } else { "changes" },
                if selection.file == 0 { "A" } else { "B" }
            );
            let origin = ui.cursor_screen_pos();
            let available = ui.content_region_avail()[0];
            let button_width = ui.calc_text_size("Apply changes")[0] + 24.0;
            let buttons_width = 2.0 * button_width + PANEL_GAP;
            ui.align_text_to_frame_padding();
            ui.text_colored(self.settings.palette.accent, &status);
            if ui.calc_text_size(&status)[0] + buttons_width + PANEL_GAP <= available {
                ui.same_line();
                ui.set_cursor_screen_pos([origin[0] + available - buttons_width, origin[1]]);
            }
            let discard =
                ui.button_with_size("Discard##cell_edits", [button_width, ui.frame_height()]);
            item_tip(
                ui,
                self.settings.tutorial_tips,
                "",
                "Drop staged edits in this dataset without writing them.",
            );
            ui.same_line_with_spacing(0.0, PANEL_GAP);
            let apply = ui.button_with_size(
                "Apply changes##cell_edits",
                [button_width, ui.frame_height()],
            );
            item_tip(
                ui,
                self.settings.tutorial_tips,
                "",
                "Write the highlighted changes to the file.",
            );
            if discard {
                self.pending_cell_edits
                    .retain(|key, _| key.file != selection.file || key.path != selection.path);
                self.cell_editor = None;
                self.cell_edit_message = None;
            }
            if apply && !discard {
                self.start_cell_edit_job(selection);
            }
        } else if elsewhere > 0 {
            ui.text(format!(
                "{elsewhere} pending {} in another dataset. Return there to apply or discard.",
                if elsewhere == 1 { "change" } else { "changes" }
            ));
        }
        if let Some(message) = &self.cell_edit_message {
            ui.text_wrapped(message);
        }
    }

    pub(super) fn draw_editable_cell(
        &mut self,
        ui: &Ui,
        key: CellKey,
        value: f64,
        fill_value: Option<f64>,
        decimals: usize,
        palette: &Palette,
        width: f32,
    ) {
        let staged = self.pending_cell_edits.get(&key).cloned();
        let cell_width = (width - 2.0 * TABLE_CELL_PADDING[0]).max(40.0);
        let cell_height = ui.frame_height();
        let editing = self
            .cell_editor
            .as_ref()
            .is_some_and(|editor| editor.key == key);
        if editing {
            let editor = self.cell_editor.as_mut().unwrap();
            if editor.focus {
                ui.set_keyboard_focus_here();
                editor.focus = false;
            }
            ui.set_next_item_width(cell_width);
            let submitted = ui
                .input_text(
                    format!("##edit_{:?}_{}_{}", key.coordinates, key.file, key.path),
                    &mut editor.text,
                )
                .auto_select_all(true)
                .enter_returns_true(true)
                .build();
            item_tip(
                ui,
                self.settings.tutorial_tips,
                "",
                "Press Enter to stage this value; use Fill, NaN, +Inf, or -Inf where supported.",
            );
            let cancelled = ui.is_item_deactivated() && !submitted;
            if submitted {
                let editor = self.cell_editor.take().unwrap();
                match self.files[key.file].validate_numeric_edit(&key.path, &editor.text) {
                    Ok(replacement) => {
                        if replacement == editor.original {
                            self.pending_cell_edits.remove(&key);
                        } else {
                            self.pending_cell_edits.insert(
                                key,
                                PendingCellEdit {
                                    original: editor.original,
                                    replacement,
                                },
                            );
                        }
                        self.cell_edit_message = None;
                    }
                    Err(err) => {
                        self.cell_edit_message = Some(err.to_string());
                        self.cell_editor = Some(CellEditor {
                            focus: true,
                            ..editor
                        });
                    }
                }
            } else if cancelled {
                self.cell_editor = None;
            }
            return;
        }
        let special = staged
            .as_ref()
            .and_then(|edit| edit.replacement.parse::<f64>().ok())
            .map_or_else(
                || SpecialValue::from_value(value, fill_value),
                |replacement| SpecialValue::from_value(replacement, fill_value),
            );
        let label = if let Some(special) = special {
            special.label().to_owned()
        } else if let Some(edit) = &staged {
            edit.replacement.clone()
        } else {
            format_value(value, decimals)
        };
        let _color =
            special.map(|special| ui.push_style_color(StyleColor::Text, special.color(palette)));
        if staged.is_some() {
            ui.table_set_cell_bg_color(
                [
                    palette.accent[0],
                    palette.accent[1],
                    palette.accent[2],
                    0.24,
                ],
                ui.table_get_column_index().unwrap(),
            );
        }
        let _button_color = ui.push_style_color(StyleColor::Button, [0.0, 0.0, 0.0, 0.0]);
        let _button_border = ui.push_style_var(StyleVar::FrameBorderSize(0.0));
        let _left_align = ui.push_style_var(StyleVar::ButtonTextAlign([0.0, 0.5]));
        let clicked = ui.button_with_size(
            format!(
                "{label}##cell_{:?}_{}_{}",
                key.coordinates, key.file, key.path
            ),
            [cell_width, cell_height],
        );
        let hovered = ui.is_item_hovered();
        if hovered {
            ui.set_mouse_cursor(Some(MouseCursor::TextInput));
        }
        item_tip(
            ui,
            self.settings.tutorial_tips,
            "",
            "Click to edit this cell. Press Enter to stage the change.",
        );
        if clicked {
            match self.files[key.file].numeric_cell(&key.path, &key.coordinates) {
                Ok(original) => {
                    let text = if let Some(special) = special {
                        special.label().to_owned()
                    } else {
                        staged
                            .as_ref()
                            .map_or_else(|| original.clone(), |edit| edit.replacement.clone())
                    };
                    let original = staged.map_or(original, |edit| edit.original);
                    self.cell_editor = Some(CellEditor {
                        key,
                        original,
                        text,
                        focus: true,
                    });
                    self.cell_edit_message = None;
                }
                Err(err) => self.cell_edit_message = Some(err.to_string()),
            }
        }
    }

    pub(crate) fn draw_pending_selection_dialog(&mut self, ui: &Ui) {
        const POPUP: &str = "Pending cell edits##switch_dataset";
        if self.queued_selection.is_some() && self.cell_edit_job.is_none() {
            ui.open_popup(POPUP);
        }
        if let Some(_popup) = ui
            .begin_modal_popup_config(POPUP)
            .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
            .begin()
        {
            let Some(current) = self.selection.clone() else {
                self.queued_selection = None;
                ui.close_current_popup();
                return;
            };
            ui.text_wrapped("This dataset has staged cell changes. Apply or discard them before switching datasets.");
            let button_width = ui.calc_text_size("Apply changes")[0] + 24.0;
            let button_size = [button_width, ui.frame_height()];
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "Apply changes##switch_dataset",
                button_size,
            ) {
                self.start_cell_edit_job(&current);
                if self.cell_edit_job.is_some() {
                    ui.close_current_popup();
                }
            }
            ui.same_line_with_spacing(0.0, PANEL_GAP);
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "Discard##switch_dataset",
                button_size,
            ) {
                self.pending_cell_edits
                    .retain(|key, _| key.file != current.file || key.path != current.path);
                self.cell_editor = None;
                self.selection = self.queued_selection.take();
                self.refresh_selection();
                ui.close_current_popup();
            }
            ui.same_line_with_spacing(0.0, PANEL_GAP);
            if draw_accent_button(
                ui,
                &self.settings.palette,
                "Cancel##switch_dataset",
                button_size,
            ) {
                self.queued_selection = None;
                ui.close_current_popup();
            }
            if let Some(message) = &self.cell_edit_message {
                ui.text_wrapped(message);
            }
        }
    }

    pub(crate) fn start_cell_edit_job(&mut self, selection: &Selection) {
        if self.cell_edit_job.is_some()
            || self.bulk_replace_job.is_some()
            || self.plugins.is_busy()
            || self.file_open_jobs.iter().any(Option::is_some)
        {
            self.cell_edit_message =
                Some("Wait for file operations to finish before applying edits.".into());
            return;
        }
        let slot = selection.file;
        let file_path = self.file_paths[slot].clone();
        if self.file_paths.iter().enumerate().any(|(other, path)| {
            other != slot
                && std::fs::canonicalize(path).ok() == std::fs::canonicalize(&file_path).ok()
        }) {
            self.cell_edit_message = Some("The same file is open in both slots. Replace the other slot before applying edits.".into());
            return;
        }
        let mut entries = self
            .pending_cell_edits
            .iter()
            .filter(|(key, _)| key.file == slot && key.path == selection.path)
            .map(|(key, edit)| (key.clone(), edit.clone()))
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.0.coordinates.cmp(&b.0.coordinates));
        if entries.is_empty() {
            return;
        }
        let keys = entries.iter().map(|(key, _)| key.clone()).collect();
        let edits = entries
            .into_iter()
            .map(|(key, edit)| NumericEdit {
                coordinates: key.coordinates,
                original: edit.original,
                replacement: edit.replacement,
            })
            .collect::<Vec<_>>();
        if let Err(err) = self.files[slot].release_for_edit() {
            self.cell_edit_message = Some(err.to_string());
            return;
        }
        let path = selection.path.clone();
        let worker_path = path.clone();
        let options = OpenOptions {
            max_depth: self.settings.tree_max_depth,
            max_objects: self.settings.tree_max_objects,
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let outcome = File::apply_numeric_edits(&file_path, &worker_path, &edits);
            let reopened =
                File::open_with_options(&file_path, options).map_err(|err| err.to_string());
            let _ = sender.send((outcome, reopened));
        });
        self.cell_editor = None;
        self.cell_edit_message = Some("Applying changes...".into());
        self.cell_edit_job = Some(CellEditJob {
            file: slot,
            path,
            restore_selection: self.selection.clone(),
            keys,
            receiver,
        });
        self.reset_file_selection();
    }

    pub(crate) fn poll_cell_edit_job(&mut self) {
        let result = self
            .cell_edit_job
            .as_ref()
            .map(|job| job.receiver.try_recv());
        match result {
            Some(Ok((outcome, reopened))) => {
                let job = self.cell_edit_job.take().unwrap();
                for key in job.keys.iter().take(outcome.applied) {
                    self.pending_cell_edits.remove(key);
                }
                match reopened {
                    Ok(file) => {
                        self.files[job.file] = file;
                        self.reset_file_selection();
                        if let Some(selection) = job.restore_selection.filter(|selection| {
                            selection.file < self.files.len()
                                && find_entry(&self.files[selection.file].tree, &selection.path)
                                    .is_some()
                        }) {
                            self.selection = Some(selection);
                            self.refresh_selection();
                        }
                        self.plugins.invalidate();
                    }
                    Err(err) => {
                        self.file_error =
                            Some(format!("Could not reopen file after editing: {err}"));
                    }
                }
                let succeeded = outcome.error.is_none();
                self.cell_edit_message = Some(match outcome.error {
                    Some(err) => format!(
                        "Applied {} of {} changes to {}: {err}",
                        outcome.applied,
                        job.keys.len(),
                        job.path
                    ),
                    None => format!("Applied {} changes to {}", outcome.applied, job.path),
                });
                if succeeded {
                    if let Some(next) = self.queued_selection.take() {
                        self.selection = Some(next);
                        self.refresh_selection();
                    }
                } else {
                    self.queued_selection = None;
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.cell_edit_job = None;
                self.cell_edit_message = Some(
                    "Edit worker stopped unexpectedly. Reload the file before retrying.".into(),
                );
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
    }
}
