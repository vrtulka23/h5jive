# World map and globe contract

## Eligibility and source mapping

- A geospatial view is enabled only when the selected dataset has usable
  latitude and longitude. Switching to a dataset without them opens Data view.
- A displayed sample, its tooltip, and its map location refer to the same
  source index and pinned spectral or time index. Scan-direction filtering is
  applied consistently to values and footprints.
- Where pixel corner bounds exist, filled rectangles or edges use those bounds
  and adjacent pixels meet according to the source geometry. Point style uses
  the central coordinates. Missing bounds do not produce invented footprints.

## World map

- Longitude wraps and latitude clips correctly at map boundaries. Zoom remains
  centered on the pointer; dragging pans the view.
- Density reduction is available to all supported dataset formats. Its setting
  controls how many source pixels contribute to a displayed cell. Zooming in
  refines toward native pixels; zooming out reduces density again without
  leaving gaps between sampled cells.

## Globe

- Dragging rotates the globe and geolocated data together. Zoom changes their
  apparent size while keeping the same geographic alignment.
- Visible pixel footprints, central points, and PICS markers stay on the
  correct side of the sphere and use the selected data plane.

## Shared display behavior

- Filled footprints and 0.5 data opacity are the initial settings when bounds
  are available. Pixel-style controls apply immediately; settings documented
  as requiring **Update** apply when it is pressed.
- A pending map or globe request keeps the last compatible image visible. A
  completed request from an earlier dataset, zoom, or scan filter does not
  replace the current one.

## Regression sequence

Use the bundled GOME-2 Band 3 sample for pixels with bounds and a dataset
without geolocation for eligibility. Switch point, edge, and filled styles;
zoom both ways; change the pinned spectral index and scan filter; rotate the
globe; then switch to the non-geospatial dataset. Verify source indexes,
coverage, controls, and the Data-view fallback.
