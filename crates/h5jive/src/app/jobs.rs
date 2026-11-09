use super::*;

impl App {
    pub(super) fn poll_heatmap_overview(
        &mut self,
        selection: &Selection,
        shape: &[usize],
        slice: &TableSlice,
        grid: [usize; 2],
    ) {
        let key = HeatmapOverviewKey {
            file: selection.file,
            path: selection.path.clone(),
            slice: slice.clone(),
            grid,
        };
        if self
            .heatmap_overview_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.heatmap_overview_cache = None;
        if self
            .heatmap_overview_job
            .as_ref()
            .is_some_and(|(running, _)| running == &key)
        {
            match self.heatmap_overview_job.as_ref().unwrap().1.try_recv() {
                Ok(image) => {
                    self.heatmap_overview_cache = Some((key, image));
                    self.heatmap_overview_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.heatmap_overview_cache =
                        Some((key, Err("Overview worker stopped".into())));
                    self.heatmap_overview_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            return;
        }
        self.heatmap_overview_job = None;
        let file_path = self.file_paths[selection.file].clone();
        let path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = open_dataset_reader(&file_path)
                .and_then(|file| load_heatmap_overview(&file, &path, &shape, &slice, grid));
            let _ = sender.send(result);
        });
        self.heatmap_overview_job = Some((key, receiver));
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
            .heatmap_range_cache
            .as_ref()
            .is_some_and(|(cached, _)| cached == &key)
        {
            return;
        }
        self.heatmap_range_cache = None;
        if self
            .heatmap_range_job
            .as_ref()
            .is_some_and(|(running, _)| running == &key)
        {
            let result = self.heatmap_range_job.as_ref().unwrap().1.try_recv();
            match result {
                Ok(range) => {
                    self.heatmap_range_cache = Some((key, range));
                    self.heatmap_range_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.heatmap_range_cache = Some((key, Err("Range worker stopped".into())));
                    self.heatmap_range_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            return;
        }
        self.heatmap_range_job = None;
        let file_path = self.file_paths[selection.file].clone();
        let path = selection.path.clone();
        let shape = shape.to_vec();
        let slice = slice.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = open_dataset_reader(&file_path)
                .and_then(|file| scan_heatmap_range(&file, &path, &shape, &slice));
            let _ = sender.send(result);
        });
        self.heatmap_range_job = Some((key, receiver));
    }

    pub(super) fn poll_statistics(&mut self, selection: &Selection) {
        if let Some(job) = &self.statistics_job {
            match job.try_recv() {
                Ok(result) => {
                    self.statistics_cache = Some(result);
                    self.statistics_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.statistics_cache = Some(Err("Statistics worker stopped".into()));
                    self.statistics_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.statistics_cache.is_none() && self.statistics_job.is_none() {
            let file_path = self.file_paths[selection.file].clone();
            let dataset_path = selection.path.clone();
            let sample_limit = self.settings.statistics_sample_values;
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let result = open_dataset_reader(&file_path).and_then(|file| {
                    file.statistics(&dataset_path, sample_limit)
                        .map_err(|err| err.to_string())
                });
                let _ = sender.send(result);
            });
            self.statistics_job = Some(receiver);
        }
    }

    pub(super) fn poll_distribution(
        &mut self,
        selection: &Selection,
        statistics: &DatasetStatistics,
    ) {
        if let Some(job) = &self.distribution_job {
            match job.try_recv() {
                Ok(result) => {
                    self.distribution_cache = Some(result);
                    self.distribution_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.distribution_cache = Some(Err("Distribution worker stopped".into()));
                    self.distribution_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.distribution_cache.is_none() && self.distribution_job.is_none() {
            let file_path = self.file_paths[selection.file].clone();
            let dataset_path = selection.path.clone();
            let statistics = statistics.clone();
            let bin_count = self.settings.histogram_bins;
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let result = open_dataset_reader(&file_path).and_then(|file| {
                    file.distribution(&dataset_path, &statistics, bin_count)
                        .map_err(|err| err.to_string())
                });
                let _ = sender.send(result);
            });
            self.distribution_job = Some(receiver);
        }
    }

    pub(super) fn refresh_selection(&mut self) {
        let details_key = self
            .selection
            .as_ref()
            .map(|selection| (selection.file, selection.path.clone()));
        if details_key != self.details_key {
            self.details_key = details_key;
            self.statistics_cache = None;
            self.statistics_job = None;
            self.distribution_cache = None;
            self.distribution_job = None;
            self.attributes_cache = None;
            self.coordinates_cache = None;
            self.coordinate_value_cache.clear();
        }
        let key = self.selection.as_ref().and_then(|selection| {
            let entry = find_entry(&self.files[selection.file].tree, &selection.path)?;
            if matches!(entry.kind, Kind::Dataset { numeric: true, .. }) {
                Some((selection.file, selection.path.clone()))
            } else {
                None
            }
        });
        let selection_changed = key != self.selection_key;
        if selection_changed {
            self.selection_key = key;
            self.volume_pan = [0.0, 0.0];
            self.profile_zoom = 1.0;
            self.heatmap_nav_override = None;
            self.heatmap_tick_view = None;
            self.region_cache.clear();
            self.heatmap_cache = None;
            self.profile_cache = None;
            self.volume_cache = None;
            self.volume_job = None;
            self.volume_raster = None;
            self.volume_raster_job = None;
            self.map_cache = None;
            self.map_job = None;
            self.axis_names = self
                .selection_key
                .as_ref()
                .and_then(|(index, path)| self.files[*index].axis_names(path).ok())
                .unwrap_or_default();
        }
        let selected_shape = self.selection.as_ref().and_then(|selection| {
            let entry = find_entry(&self.files[selection.file].tree, &selection.path)?;
            match &entry.kind {
                Kind::Dataset {
                    shape,
                    numeric: true,
                } if (1..=4).contains(&shape.len()) => Some(shape.as_slice()),
                _ => None,
            }
        });
        if selection_changed {
            let first_is_time = self
                .axis_names
                .first()
                .is_some_and(|name| name.to_ascii_lowercase().contains("time"))
                || self.selection.as_ref().is_some_and(|selection| {
                    self.files[selection.file]
                        .coordinate_axes(&selection.path)
                        .ok()
                        .and_then(|axes| axes.into_iter().next())
                        .is_some_and(|axis| {
                            axis.name.to_ascii_lowercase().contains("time")
                                || axis.units.is_some_and(|units| {
                                    units.to_ascii_lowercase().contains("since")
                                })
                        })
                });
            self.draft_table = selected_shape.map(|shape| {
                TableSlice::with_leading_time(shape, first_is_time, self.settings.pinned_default)
            });
            self.applied_table = self.draft_table.clone();
            self.draft_slice = selected_shape.map(|shape| {
                TableSlice::with_leading_time(shape, first_is_time, self.settings.pinned_default)
            });
            self.applied_slice = self.draft_slice.clone();
            self.draft_profile =
                selected_shape.map(|shape| ProfileSlice::new(shape, self.settings.pinned_default));
            self.applied_profile = self.draft_profile.clone();
            self.draft_volume = selected_shape
                .map(|shape| VolumeSlice::new(shape, first_is_time, self.settings.pinned_default));
            self.applied_volume = self.draft_volume.clone();
            self.draft_volume_opacity = self.volume_opacity;
            self.draft_volume_log_scale = self.volume_log_scale;
            self.table_column_page = 0;
        }
    }
}
