use super::*;

impl App {
    /// Invalidate the selected dataset's derived data without changing view controls.
    pub(super) fn reload_selected_dataset(&mut self) {
        let Some(selection) = self.session.selection.clone() else {
            return;
        };
        self.dataset
            .tree_geolocation_cache
            .remove(&(selection.file, selection.path.clone()));
        self.dataset.axis_names =
            match self.session.files[selection.file].axis_names(&selection.path) {
                Ok(names) => names,
                Err(error) => {
                    self.show_notice(
                        NoticeKind::Error,
                        format!("Could not reload dataset axis metadata: {error}"),
                    );
                    Vec::new()
                }
            };
        self.dataset.details_key = None;
        self.clear_selected_dataset_views();
        self.geo.globe_auto_center_source = None;
        self.geo.globe_rotation = None;
    }

    /// Clear metadata derived from the currently selected file entry.
    pub(super) fn clear_selection_details(&mut self) {
        self.dataset.details_default_pending = true;
        self.dataset.statistics_cache = None;
        self.dataset.statistics_job.cancel();
        self.dataset.distribution_cache = None;
        self.dataset.distribution_job.cancel();
        self.dataset.attributes_cache = None;
        self.dataset.coordinates_cache = None;
        self.dataset.coordinate_value_cache.clear();
    }

    /// Invalidate data derived from the selected numeric dataset, including in-flight work.
    pub(super) fn clear_selected_dataset_views(&mut self) {
        self.workspace
            .view_load_generation
            .fetch_add(1, Ordering::Relaxed);
        self.dataset.region_cache.clear();
        self.heatmap.cache = None;
        self.heatmap.tiles.clear();
        self.heatmap.tile_job.cancel();
        self.heatmap.overview_cache = None;
        self.heatmap.overview_job.cancel();
        self.heatmap.view_overview_cache = None;
        self.heatmap.view_overview_job.cancel();
        self.profile.cache = None;
        self.profile.job.cancel();
        self.volume.cache = None;
        self.volume.overview_cache = None;
        self.volume.refined_cache.clear();
        self.volume.job.cancel();
        self.volume.raster = None;
        self.volume.overview_raster = None;
        self.volume.raster_job = None;
        self.geo.map_cache = None;
        self.geo.map_job.cancel();
        self.geo.map_plane = None;
        self.geo.map_load_generation.fetch_add(1, Ordering::Relaxed);
    }
}
