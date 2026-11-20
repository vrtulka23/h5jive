use super::*;

impl App {
    /// Invalidate the selected dataset's derived data without changing view controls.
    pub(super) fn reload_selected_dataset(&mut self) {
        let Some(selection) = self.selection.as_ref() else {
            return;
        };
        self.tree_geolocation_cache
            .remove(&(selection.file, selection.path.clone()));
        self.axis_names = self.files[selection.file]
            .axis_names(&selection.path)
            .unwrap_or_default();
        self.details_key = None;
        self.clear_selected_dataset_views();
        self.globe_auto_center_source = None;
        self.globe_rotation = None;
    }

    /// Clear metadata derived from the currently selected file entry.
    pub(super) fn clear_selection_details(&mut self) {
        self.details_default_pending = true;
        self.statistics_cache = None;
        self.statistics_job = None;
        self.distribution_cache = None;
        self.distribution_job = None;
        self.attributes_cache = None;
        self.coordinates_cache = None;
        self.coordinate_value_cache.clear();
    }

    /// Invalidate data derived from the selected numeric dataset, including in-flight work.
    pub(super) fn clear_selected_dataset_views(&mut self) {
        self.view_load_generation.fetch_add(1, Ordering::Relaxed);
        self.region_cache.clear();
        self.heatmap_cache = None;
        self.heatmap_tiles.clear();
        self.heatmap_tile_job = None;
        self.heatmap_overview_cache = None;
        self.heatmap_overview_job = None;
        self.heatmap_view_overview_cache = None;
        self.heatmap_view_overview_job = None;
        self.profile_cache = None;
        self.profile_job = None;
        self.volume_cache = None;
        self.volume_overview_cache = None;
        self.volume_refined_cache.clear();
        self.volume_job = None;
        self.volume_raster = None;
        self.volume_overview_raster = None;
        self.volume_raster_job = None;
        self.map_cache = None;
        self.map_job = None;
        self.map_plane = None;
        self.map_load_generation.fetch_add(1, Ordering::Relaxed);
    }
}
