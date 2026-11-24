# Plugin Lab

Plugin Lab is H5Jive's collection of optional, source-level extensions. The
path comparison plugin compares paths in the loaded files A and B. It labels a
path **Same** when object kind, dataset shape and datatype, and displayed
attribute values match, **Different** when they do not, and **Only A/B** when
the path is absent from one file. Dataset values are not compared. Attribute comparison uses
the values exposed by H5Jive's metadata reader; large or unsupported attribute
values may be summarized. Double-click a dataset in either column to open it in
the Data inspector; compatible datasets can then use the other core views.

## Try the demo

From the repository root:

```sh
cargo run -p h5jive --example generate_plugin_demo
cargo run -p h5jive -- examples/plugins/path_comparison/plugin_demo_a.hdf5 examples/plugins/path_comparison/plugin_demo_b.hdf5
```

Open **Plugin Lab** in the top mode bar. Both files are loaded through H5Jive's
normal file slots, so plugins use the same file trees and dataset access as the
inspector. Choose a plugin using its icon in the vertical tab rail on the left.
Use **Help → Help for current section** for the active plugin's instructions.
The A and B panels share one scrollable row grid, keeping matching paths aligned
as you scroll. Status icons indicate a match, a metadata difference, a path
present on only one side, or metadata that could not be read. Hover an icon for
the fields that differ. Use the count buttons to filter paths by status. Path
details start collapsed; click a path or chevron on either side to reveal
metadata on both sides. The metadata scan runs in the background.

The path comparison plugin can copy missing groups and datasets in either
direction. Choose **Copy missing A → B** or **Copy missing B → A**, then review
the count in the confirmation window. Click a missing-path status icon to copy
one path instead. The copy runs in the background and reloads the destination
file afterward. Existing paths, including paths with different metadata, are
left untouched. A copied group includes its children and attributes. HDF5
object copying preserves links as links and includes referenced objects needed
by the copy. Check external links before relying on a synchronized file as a
standalone copy. When copying one nested dataset, any missing parent groups
are created without their source attributes; copy the parent group to preserve
those attributes.

Click a **Different** status icon to resolve that path by replacing the whole
object in either direction. The prompt shows the path and differing metadata.
For datasets, replacement also changes values, even though this plugin does
not compare values. Replacing a group removes destination-only children. The
new object is staged before the destination link is changed; the app attempts
to restore the old link if the swap fails. This replaces the whole object
rather than merging individual attributes. References elsewhere in the
destination file to the old object may need review.

## Compare dataset values

The second plugin compares numerical datasets from either loaded file: A/B,
A/A, and B/B are all supported. It shows only datasets with the same shape as
the first selection in the second picker. Choose **Left − Right** or
**Left ÷ Right**, then select two displayed dimensions; every other dimension
gets a pinned index. The 2D view uses the same bounded sampling and heatmap
drawing components as the inspector while keeping its own selection and cache.
Invalid values, fill values, and ratio cells with a zero denominator are
excluded. For large planes, the preview samples matching positions from both
datasets, up to `plugins.value_comparison.preview_max_cells` in DIPL.
The buttons beside the heatmap filter positive, zero, and negative finite
results, as well as NaN, fill, and infinite values.

The [value comparison demo](../../examples/plugins/value_comparison/README.md)
contains smooth peaks that shift between files, visible special-value patches,
a 3D array for pinning, and an incompatible shape. Generate it with:

```sh
cargo run -p h5jive --example generate_value_comparison_demo
cargo run -p h5jive -- examples/plugins/value_comparison/value_demo_a.hdf5 examples/plugins/value_comparison/value_demo_b.hdf5
```

## Create a dataset

The Dataset manipulation plugin has its own dataset selection and slice controls.
Choose **Slice** and set a start (inclusive), stop (exclusive), and step for
each dimension, or choose **Add**, **Subtract**, **Multiply**, or **Divide** for two datasets
of identical shape. The result is written under a new dataset path in file A or
B, or in a new HDF5 file. Existing datasets and files are not overwritten.
Slices preserve the source numeric datatype. Integer additions, differences,
and products use signed 64-bit values when they fit; integer ratios use 64-bit floating point values.
Two 32-bit floating point inputs produce a 32-bit result, while operations
involving 64-bit floating point values produce a 64-bit result. Source metadata
attributes are not copied. The plugin reads HDF5 and NetCDF-4 inputs; classic
NetCDF inputs need conversion to HDF5 first.

The [dataset manipulation demo](../../examples/plugins/dataset_manipulation/README.md)
shows reproducible slices and arithmetic operations.

## Spatiotemporal subset

The Spatiotemporal subset plugin uses Dataset Browser's indexed products. Select
a source, product date or orbit range, geographic bounds, and numeric variables.
Its coverage preview opens in a popup with a static world map and two opposite
globe hemispheres. Enter the spatial bounds in the input fields. **Native products**
exports cropped swaths in separate groups of one NetCDF-4 file, keeping their
coordinates, numeric time axis, variable attributes, and available pixel bounds
and scan direction. **Combined time series** joins compatible grids or swaths
along a shared leading dimension. Swaths keep latitude, longitude, pixel bounds,
and scan direction per sample; differing scan lengths are padded with missing
values and recorded in `row_count` and `column_count`. A CF time coordinate is reused and normalized
to seconds since 1970-01-01; products are ordered by time, and overlaps are
rejected. When there is no time coordinate, name the leading index dimension
(for example, `product`). The optional group name places the series under one
NetCDF group; leaving it blank writes variables at the root. Regular grid
coordinates, selected variable units, and calendars must match. **Gridded mosaic** combines valid samples on one regular
grid using Mean, Newest, Oldest, Minimum, or Maximum. Mean gives every valid
sample equal weight. Newest and Oldest follow Dataset Browser date/orbit order
and CF time within each product. The companion count records all valid samples,
and the output includes grid cell bounds. Both modes read spatial rows in bounded chunks and create a
new file without overwriting an existing one.

The mosaic methods do not apply quality flags or uncertainty weights. For
satellite radiance, use scientifically comparable channels and observation
conditions; a quality-based pixel choice is a separate method.

The first version supports NetCDF products with 2D swath coordinates or 1D
regular latitude and longitude axes, numeric 2D fields, and 3D fields with a
leading CF time coordinate. Native export keeps every channel of 4D
geolocated fields such as GOME-2 radiance, while mosaic export averages each
channel separately. Both outputs can be reopened in World map and Globe view.
Mosaic cells have new footprints and do not carry source scan direction. The
plugin does not reproject curvilinear grids, preserve source numeric
storage types, or write into existing A/B files. See the
[subset demo](../../examples/plugins/spatiotemporal_subset/README.md) for a
reproducible source and export.

Filesystem paths throughout H5Jive use the same text field with an attached
folder button. Typing remains available; the button opens the Files picker.
For output paths, the picker chooses a folder and filename and returns to the
calling tool before saving.

## Add a plugin

Coding agents should also read the [plugin agent notes](https://github.com/vrtulka23/h5jive/blob/main/crates/h5jive/src/plugins/AGENTS.md)
before editing the plugin registry or adding a module.

1. Download or clone [the source code](https://github.com/vrtulka23/h5jive).
2. Create a module under `crates/h5jive/src/plugins/`. Read data through
   `PluginContext.files`, inspect `PluginContext.selection` when needed, and
   return a `Selection` to open a dataset in the core
   inspector. Keep plugin UI and analysis state in the module.
3. Register it in `crates/h5jive/src/plugins/mod.rs` with a unique key, icon,
   title, enabled setting, and draw function. The icon appears in the left tab
   rail when the plugin is enabled.
4. Give it a dedicated schema in `settings/runtime/schemas/plugins/` and a settings
   file in `settings/runtime/plugins/`. Add its `enabled` switch to the central
   `settings/runtime/plugins.dip` and wire the values through `Settings`.
5. Add a reproducible demo in `examples/`, run `cargo test -p h5jive`, and
   submit a pull request with a short description and screenshot. The
   [Plugin Lab catalog](https://github.com/vrtulka23/h5jive/tree/main/plugins)
   is the project's small app store for shared source plugins.

The current design compiles plugins with H5Jive. It is a source extension
point, not a runtime loader for external binaries.

### A prompt for an AI coding agent

If you are a scientist new to Rust, start with a precise task and a small
sample file. For example:

> In H5Jive's Plugin Lab, add a plugin that computes the mean of the selected
> numeric HDF5 dataset and shows the result beside its path. Use the existing
> file and bounded region APIs; keep the code in `src/plugins`. Add a separate
> DIPL schema and settings file, a central enabled switch, a tiny example in
> `examples/`, and a test. Explain how to run the demo.

Check the output on a dataset whose mean you know, and review the code before
submitting it.
