use super::*;

impl App {
    pub(super) fn refinement_settled(activity: Option<Instant>) -> bool {
        activity.is_none_or(|at| at.elapsed() >= REFINEMENT_IDLE)
    }

    pub(super) fn note_heatmap_navigation(&mut self) {
        self.heatmap.navigation_at = Some(Instant::now());
        let tile_cancelled = self.heatmap.tile_job.cancel();
        let view_cancelled = self.heatmap.view_overview_job.cancel();
        if tile_cancelled || view_cancelled {
            self.heatmap
                .refine_generation
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn note_volume_zoom(&mut self) {
        self.volume.navigation_at = Some(Instant::now());
        let overview = self.volume.applied_slice.as_ref();
        if self
            .volume
            .job
            .key()
            .is_some_and(|key| Some(&key.slice) != overview)
        {
            self.volume
                .refine_generation
                .fetch_add(1, Ordering::Relaxed);
            self.volume.job.cancel();
        }
        if self.volume.raster_job.take().is_some() {
            self.volume
                .raster_generation
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn switch_view(&mut self, view: ViewMode) {
        if self.workspace.view == view {
            return;
        }
        let keep_map_job = matches!(self.workspace.view, ViewMode::WorldMap | ViewMode::Globe)
            && matches!(view, ViewMode::WorldMap | ViewMode::Globe);
        self.workspace
            .view_load_generation
            .fetch_add(1, Ordering::Relaxed);
        self.heatmap.tile_job.cancel();
        self.heatmap.overview_job.cancel();
        self.heatmap.view_overview_job.cancel();
        self.heatmap.range_job.cancel();
        self.profile.job.cancel();
        self.volume.job.cancel();
        self.volume.raster_job = None;
        if !keep_map_job {
            self.geo.map_job.cancel();
            self.geo.map_load_generation.fetch_add(1, Ordering::Relaxed);
        }
        self.heatmap.navigation_at = None;
        self.volume.navigation_at = None;
        self.workspace.view = view;
        if view != ViewMode::Globe {
            self.geo.globe_rotation = None;
        }
    }

    pub(super) fn poll_heatmap_tile(
        &mut self,
        key: HeatmapKey,
        shape: &[usize],
        replacement: f64,
        allow_start: bool,
    ) {
        if let Some((running, image)) = self
            .heatmap
            .tile_job
            .poll("Could not load 2D tile: worker stopped unexpectedly.")
        {
            let image = image.map(Arc::new);
            if let Err(error) = &image {
                self.session
                    .notices
                    .error(format!("Could not load 2D tile: {error}"));
            }
            if let Ok(data) = &image {
                self.heatmap.tiles.retain(|(cached, _)| cached != &running);
                self.heatmap.tiles.push((running.clone(), Arc::clone(data)));
                if self.heatmap.tiles.len() > 4 {
                    self.heatmap.tiles.remove(0);
                }
            }
            self.heatmap.cache = Some((running, image));
        }
        if self
            .heatmap
            .cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        if let Some((_, image)) = self.heatmap.tiles.iter().find(|(cached, _)| cached == &key) {
            self.heatmap.cache = Some((key, Ok(Arc::clone(image))));
            return;
        }
        if self.heatmap.tile_job.is_busy() || !allow_start {
            return;
        }
        let file_path = self.session.file_paths[key.file].clone();
        let dataset_path = key.path.clone();
        let slice = key.slice.clone();
        let bounds = key.bounds;
        let shape = shape.to_vec();
        let generation = Arc::clone(&self.workspace.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        let refinement = Arc::clone(&self.heatmap.refine_generation);
        let expected_refinement = refinement.load(Ordering::Relaxed);
        self.heatmap.tile_job.start(key, move |job_cancelled| {
            let cancelled = || {
                job_cancelled()
                    || generation.load(Ordering::Relaxed) != expected
                    || refinement.load(Ordering::Relaxed) != expected_refinement
            };
            if cancelled() {
                return Err("2D tile work was cancelled".into());
            }
            open_dataset_reader(&file_path).and_then(|file| {
                load_heatmap_tile_with_cancel(
                    &file,
                    &dataset_path,
                    &shape,
                    &slice,
                    bounds,
                    replacement,
                    &cancelled,
                )
            })
        });
    }

    pub(super) fn poll_heatmap_overview(
        &mut self,
        selection: &Selection,
        shape: &[usize],
        slice: &TableSlice,
        grid: [usize; 2],
    ) {
        let bounds = [0, slice.rows(shape), 0, slice.columns(shape)];
        let key = HeatmapOverviewKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
            bounds,
            grid,
        };
        if self
            .heatmap
            .overview_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.heatmap.overview_cache = None;
        if self.heatmap.overview_job.key() == Some(&key) {
            if let Some(error) = self
                .heatmap
                .overview_job
                .finish_into(&mut self.heatmap.overview_cache, "Overview worker stopped")
            {
                self.session
                    .notices
                    .error(format!("Could not load 2D overview: {error}"));
            }
            return;
        }
        self.heatmap.overview_job.cancel();
        let file_path = self.session.file_paths[selection.file].clone();
        let path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let generation = Arc::clone(&self.workspace.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        self.heatmap.overview_job.start(key, move |job_cancelled| {
            let cancelled = || job_cancelled() || generation.load(Ordering::Relaxed) != expected;
            if cancelled() {
                return Err("2D overview work was cancelled".into());
            }
            open_dataset_reader(&file_path).and_then(|file| {
                load_heatmap_overview_with_cancel(
                    &file, &path, &shape, &slice, bounds, grid, &cancelled,
                )
            })
        });
    }

    pub(super) fn poll_heatmap_view_overview(
        &mut self,
        selection: &Selection,
        shape: &[usize],
        slice: &TableSlice,
        bounds: [usize; 4],
        grid: [usize; 2],
        allow_start: bool,
    ) {
        let key = HeatmapOverviewKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
            bounds,
            grid,
        };
        if self
            .heatmap
            .view_overview_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.heatmap.view_overview_cache = None;
        if self.heatmap.view_overview_job.key() == Some(&key) {
            if let Some(error) = self.heatmap.view_overview_job.finish_into(
                &mut self.heatmap.view_overview_cache,
                "2D view worker stopped",
            ) {
                self.session
                    .notices
                    .error(format!("Could not load 2D view: {error}"));
            }
            return;
        }
        if self.heatmap.view_overview_job.cancel() {
            self.heatmap
                .refine_generation
                .fetch_add(1, Ordering::Relaxed);
        }
        if !allow_start {
            return;
        }
        let file_path = self.session.file_paths[selection.file].clone();
        let path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let generation = Arc::clone(&self.workspace.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        let refinement = Arc::clone(&self.heatmap.refine_generation);
        let expected_refinement = refinement.load(Ordering::Relaxed);
        self.heatmap
            .view_overview_job
            .start(key, move |job_cancelled| {
                let cancelled = || {
                    job_cancelled()
                        || generation.load(Ordering::Relaxed) != expected
                        || refinement.load(Ordering::Relaxed) != expected_refinement
                };
                if cancelled() {
                    return Err("2D view work was cancelled".into());
                }
                open_dataset_reader(&file_path).and_then(|file| {
                    load_heatmap_overview_with_cancel(
                        &file, &path, &shape, &slice, bounds, grid, &cancelled,
                    )
                })
            });
    }

    pub(super) fn poll_heatmap_range(
        &mut self,
        selection: &Selection,
        shape: &[usize],
        slice: &TableSlice,
    ) {
        if !self.settings.heatmap_auto_range {
            return;
        }
        let key = HeatmapRangeKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
        };
        if self
            .heatmap
            .range_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.heatmap.range_cache = None;
        if self.heatmap.range_job.key() == Some(&key) {
            if let Some(error) = self
                .heatmap
                .range_job
                .finish_into(&mut self.heatmap.range_cache, "Range worker stopped")
            {
                self.session
                    .notices
                    .error(format!("Could not scan 2D range: {error}"));
            }
            return;
        }
        self.heatmap.range_job.cancel();
        let file_path = self.session.file_paths[selection.file].clone();
        let path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let generation = Arc::clone(&self.workspace.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        self.heatmap.range_job.start(key, move |job_cancelled| {
            let cancelled = || job_cancelled() || generation.load(Ordering::Relaxed) != expected;
            if cancelled() {
                return Err("2D range work was cancelled".into());
            }
            open_dataset_reader(&file_path).and_then(|file| {
                scan_heatmap_range_with_cancel(&file, &path, &shape, &slice, &|| cancelled())
            })
        });
    }

    pub(super) fn poll_profile(
        &mut self,
        selection: &Selection,
        shape: &[usize],
        slice: &ProfileSlice,
    ) {
        let key = ProfileKey {
            selection: selection.clone(),
            slice: slice.clone(),
            visibility: self.workspace.value_visibility,
        };
        if self.profile.job.key() == Some(&key) {
            match self
                .profile
                .job
                .poll("Profile worker stopped unexpectedly.")
            {
                Some((_, JobEvent::Progress(data))) => {
                    self.profile.cache = Some((key, Ok(data)));
                }
                Some((_, JobEvent::Finished(result))) => {
                    if let Err(error) = &result {
                        self.session
                            .notices
                            .error(format!("Could not load profile: {error}"));
                    }
                    self.profile.cache = Some((key, result));
                }
                None => {}
            }
            return;
        }
        if self
            .profile
            .cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.profile.cache = None;
        if self.profile.job.cancel() {
            self.profile.load_generation.fetch_add(1, Ordering::Relaxed);
        }
        let file_path = self.session.file_paths[selection.file].clone();
        let dataset_path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let visibility = self.workspace.value_visibility;
        let generation = Arc::clone(&self.workspace.view_load_generation);
        let expected = generation.load(Ordering::Relaxed);
        let profile_generation = Arc::clone(&self.profile.load_generation);
        let expected_profile = profile_generation.load(Ordering::Relaxed);
        self.profile.job.start(key, move |job_cancelled, progress| {
            let cancelled = || {
                job_cancelled()
                    || generation.load(Ordering::Relaxed) != expected
                    || profile_generation.load(Ordering::Relaxed) != expected_profile
            };
            if cancelled() {
                return Err("Profile work was cancelled".into());
            }
            let file = open_dataset_reader(&file_path)?;
            if profile_needs_preview(&shape, &slice) {
                let data = load_profile_preview_with_cancel(
                    &file,
                    &dataset_path,
                    &shape,
                    &slice,
                    &visibility,
                    PROFILE_OVERVIEW_SAMPLE_BUDGET,
                    &cancelled,
                )?;
                if !progress(data) {
                    return Err("Profile work was cancelled".into());
                }
            }
            if cancelled() {
                return Err("Profile work was cancelled".into());
            }
            load_profile_with_cancel(
                &file,
                &dataset_path,
                &shape,
                &slice,
                &visibility,
                &cancelled,
            )
        });
    }

    pub(super) fn poll_statistics(&mut self, selection: &Selection) {
        if let Some((key, result)) = self
            .dataset
            .statistics_job
            .poll("Statistics worker stopped")
        {
            if &key == selection {
                if let Err(error) = &result {
                    self.session
                        .notices
                        .error(format!("Could not calculate statistics: {error}"));
                }
                self.dataset.statistics_cache = Some(result);
            }
        }
        if self.dataset.statistics_cache.is_none() && !self.dataset.statistics_job.is_busy() {
            let file_path = self.session.file_paths[selection.file].clone();
            let dataset_path = selection.path.clone();
            let sample_limit = self.settings.statistics_sample_values;
            self.dataset
                .statistics_job
                .start(selection.clone(), move |_| {
                    open_dataset_reader(&file_path).and_then(|file| {
                        file.statistics(&dataset_path, sample_limit)
                            .map_err(|err| err.to_string())
                    })
                });
        }
    }

    pub(super) fn poll_distribution(
        &mut self,
        selection: &Selection,
        statistics: &DatasetStatistics,
    ) {
        if let Some((key, result)) = self
            .dataset
            .distribution_job
            .poll("Distribution worker stopped")
        {
            if &key == selection {
                if let Err(error) = &result {
                    self.session
                        .notices
                        .error(format!("Could not calculate distribution: {error}"));
                }
                self.dataset.distribution_cache = Some(result);
            }
        }
        if self.dataset.distribution_cache.is_none() && !self.dataset.distribution_job.is_busy() {
            let file_path = self.session.file_paths[selection.file].clone();
            let dataset_path = selection.path.clone();
            let statistics = statistics.clone();
            let bin_count = self.settings.histogram_bins;
            self.dataset
                .distribution_job
                .start(selection.clone(), move |_| {
                    open_dataset_reader(&file_path).and_then(|file| {
                        file.distribution(&dataset_path, &statistics, bin_count)
                            .map_err(|err| err.to_string())
                    })
                });
        }
    }

    pub(super) fn refresh_selection(&mut self) {
        let details_key = self.session.selection.clone();
        let details_changed = details_key != self.dataset.details_key;
        if details_changed {
            self.dataset.details_key = details_key;
            let selected_raw_bytes = self.session.selection.as_ref().and_then(|selection| {
                self.session
                    .files
                    .get(selection.file)?
                    .dataset_info(&selection.path)
                    .ok()
                    .map(|info| info.byte_size)
            });
            self.resource_monitor
                .set_selected_raw_bytes(selected_raw_bytes);
            self.clear_selection_details();
        }
        let key = self.session.selection.as_ref().and_then(|selection| {
            let entry = find_entry(&self.session.files[selection.file].tree, &selection.path)?;
            if matches!(entry.kind, Kind::Dataset { numeric: true, .. }) {
                Some(selection.clone())
            } else {
                None
            }
        });
        let selection_changed = key != self.dataset.selection_key;
        if selection_changed {
            self.dataset.selection_key = key;
            self.volume.pan = [0.0, 0.0];
            self.profile.zoom = 1.0;
            self.heatmap.nav_override = None;
            self.heatmap.tick_view = None;
            self.heatmap.navigation_at = None;
            self.volume.navigation_at = None;
            self.clear_selected_dataset_views();
            self.dataset.axis_names = match self.dataset.selection_key.clone() {
                Some(selection) => {
                    match self.session.files[selection.file].axis_names(&selection.path) {
                        Ok(names) => names,
                        Err(error) => {
                            self.show_notice(
                                NoticeKind::Error,
                                format!("Could not read dataset axis metadata: {error}"),
                            );
                            Vec::new()
                        }
                    }
                }
                None => Vec::new(),
            };
        }
        let selected_shape = self.session.selection.as_ref().and_then(|selection| {
            let entry = find_entry(&self.session.files[selection.file].tree, &selection.path)?;
            match &entry.kind {
                Kind::Dataset {
                    shape,
                    numeric: true,
                } if (1..=4).contains(&shape.len()) => Some(shape.clone()),
                _ => None,
            }
        });
        if selection_changed {
            let mut first_is_time = self
                .dataset
                .axis_names
                .first()
                .is_some_and(|name| name.to_ascii_lowercase().contains("time"));
            if !first_is_time {
                if let Some(selection) = self.session.selection.clone() {
                    self.ensure_coordinates_loaded(&selection);
                    first_is_time = self
                        .dataset
                        .coordinates_cache
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .and_then(|axes| axes.first())
                        .is_some_and(|axis| {
                            axis.name.to_ascii_lowercase().contains("time")
                                || axis.units.as_ref().is_some_and(|units| {
                                    units.to_ascii_lowercase().contains("since")
                                })
                        });
                }
            }
            self.dataset.draft_table = selected_shape.as_deref().map(|shape| {
                TableSlice::with_leading_time(shape, first_is_time, self.settings.pinned_default)
            });
            self.dataset.applied_table = self.dataset.draft_table.clone();
            self.heatmap.draft_slice = selected_shape.as_deref().map(|shape| {
                TableSlice::for_heatmap(shape, first_is_time, self.settings.pinned_default)
            });
            self.heatmap.applied_slice = self.heatmap.draft_slice.clone();
            self.profile.draft_slice = selected_shape
                .as_deref()
                .map(|shape| ProfileSlice::new(shape, first_is_time, self.settings.pinned_default));
            self.profile.applied_slice = self.profile.draft_slice.clone();
            self.volume.draft_slice = selected_shape
                .as_deref()
                .map(|shape| VolumeSlice::new(shape, first_is_time, self.settings.pinned_default));
            self.volume.applied_slice = self.volume.draft_slice.clone();
            self.volume.draft_opacity = self.volume.opacity;
            self.volume.draft_log_scale = self.volume.log_scale;
            self.dataset.table_column_page = 0;
        }
        if details_changed
            && matches!(self.workspace.view, ViewMode::WorldMap | ViewMode::Globe)
            && self.selected_geo_source().is_none()
        {
            self.switch_view(ViewMode::Data);
        }
    }
}
