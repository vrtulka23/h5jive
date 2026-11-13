# Exploring data

## Dataset tree and details

Use the File A and File B tabs in the left tree, expand groups, and select a
dataset to inspect it. The details
panel reports its shape, storage size, datatype, fill value, summary statistics,
and attributes. Dataset and group markers indicate whether geographic
coordinates were discovered.

The Statistics tab shows population variance and standard deviation for finite,
non-fill values. Count rows include each count's percentage of all dataset
elements. The Attributes tab resolves HDF5 `DIMENSION_LIST` references to axis
names when available.

The Statistics tab counts outliers outside the 1.5×IQR fences: values below
Q1 − 1.5×(Q3 − Q1) or above Q3 + 1.5×(Q3 − Q1). Fill values, NaN, and infinities
are excluded from quartiles and the outlier count. Quartiles are exact up to
`viewer.statistics.sample_values` valid values and approximate above that
limit; the outlier count still scans every valid value.

Large files open in the background. The shared activity indicator appears while
data is being loaded or scanned.

The value buttons in the right panel filter positive, zero, negative, NaN,
infinite, and fill values in plot views. They sit above the zoom and legend
controls. **All** shows every value class, then changes to **None** to hide them
all. The data table always shows all
values. The colored buttons also identify the special-value colors.

## Views

- **Data table:** inspect values by row and column, reorder axes, and pin the
  remaining dimensions to selected indexes. When the first dimension is time,
  the table initially shows the next two dimensions and pins the time index.
  If only one dimension remains, it appears as a single column.
- **Profiles:** plot a single line from the selected dataset against an index
  axis while every other dimension is pinned. Choose a comparison axis and
  indexes to plot multiple lines from that same dataset. When the first
  dimension is time, the initial plot axis is the second dimension and time
  starts pinned.
- **2D slice:** choose the displayed axes and pinned dimensions; adjust the
  colormap and value range. Large arrays first show a sampled overview; visible
  zoom tiles load in the background. Automatic colors use the overview range
  for these arrays, avoiding a second full-plane read.
- **3D projection:** view voxel data with axis remapping, index ranges, opacity,
  logarithmic coloring, and preset orientations. Left drag rotates the object;
  right or middle drag, or Shift+left drag, moves it. Use **Center object** to
  reset its position. A perspective camera makes near faces appear larger than
  distant faces. The displayed side lengths follow each axis's pixel count.
  The last sharp image stays visible while a new angle renders, and the
  completed image uses native voxel values when the selected volume fits the
  exact loader. Larger selections use a bounded sampled preview, labeled in
  the view; crop the axis ranges to inspect native voxels. The preview limit is
  `viewer.views.spatial_3d.preview_max_voxels` in DIPL. Automatic colors use the central
  5–95% of ordinary voxel values so rare extremes do not flatten the colors;
  the colorbar shows that same display range. Fill values start hidden in this
  view; NaN and infinite values start visible. Use the value buttons to change
  their visibility. Special values occupy their native voxels and are composited
  along the ray with the same opacity as ordinary values. Near voxels keep their
  full color, while distant voxels are dimmed for depth. Lower opacity to see
  structures behind the front layer.
- **Flat world map:** view geolocated samples as point locations, pixel edges,
  or filled pixel footprints. Footprint modes require latitude and longitude
  bounds. Filled footprints and 50% opacity are the initial settings when bounds
  are available. Icon buttons to the left of **Update** switch pixel styles immediately, and
  **Dummy color** offers a color picker. Wheel zoom is centered on the pointer;
  drag to pan. Use the zoom arrows or select a PICS site to zoom in. **Data opacity** lets the Earth image show through the
  observations. Press **Update** to apply color, opacity, and PICS visibility;
  for 4D data it also loads the chosen pinned fourth-axis plane.
- **Globe:** rotate the geolocated samples and PICS regions on a sphere by
  dragging, and zoom with the wheel or zoom arrows. Select a PICS site to center
  it. Filled footprints and 50% opacity are the initial settings when bounds
  are available. **Data opacity** controls the transparency of the observations. Pixel
  style buttons apply immediately; color, opacity, PICS, and data controls apply
  when **Update** is clicked.

Each plot view has Reset and zoom controls below the value buttons and above
its legend. They act immediately;
Reset restores the initial plot limits or camera position and orientation.

Choose **Files > Save current view...** to save the displayed view as a PNG or
JPEG image. The export includes a caption with the view, source file, dataset
path, dataset shape, and relevant axes or map settings. Enter a new filename ending in `.png`, `.jpg`, or
`.jpeg`.

Special values such as NaN, infinities, and fill values use the colors shared
with the 2D view. In the bundled Band 3 sample, select
`MODE_NADIR/BAND_3/OBSERVATIONS/radiance`. The map and globe use the accompanying
latitude, longitude, four-corner bounds, and scan direction datasets.
