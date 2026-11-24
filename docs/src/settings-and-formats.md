# Settings and data formats

## DIPL settings

The [settings tree](../../settings/README.md) has a README in each source
directory, connecting the two DIPL projects, their schemas, and their values.

**Tutorial tips** are on by default for new installations. They add short
instructions to tooltips on interactive controls. Use **Help → Tutorial tips**
to turn them off or on; the choice is saved in `settings/runtime/overrides.dip`
(or the installed settings directory). Normal tooltips remain when tutorial
tips are off. You can also set `viewer.application.tutorial_tips = false` in
that override file.

The editable project is `settings/runtime/DIPfile`. It loads the viewer
configuration, appearance profiles, PICS catalog, and their schemas. Values
such as map zoom, point limits, color ranges, and view defaults live in DIPL.
Constraints that DIPL can express belong in schemas rather than duplicated
Rust checks. The dataset-format schema checks scan direction options and the
syntax and required presence of scan flag lists.
Rust still converts those lists to integers and checks that forward and
backward codes do not overlap.

`settings/build/DIPfile` holds values compiled into the executable, including
Plugin Lab icon rail dimensions. `settings/build/resource_limits.dip` sets
ceilings for file tree depth and object count, statistics sampling, 2D slice
sampling, and 3D preview sampling. The corresponding runtime settings remain
editable up to those ceilings; exceeding one produces a startup error. Build
settings never read
`settings/runtime/overrides.dip`; release packages contain only the runtime
project.

The main files are:

- `settings/runtime/viewer.dip` and `settings/runtime/schemas/viewer.dip`
- `settings/runtime/pics/*.dip` and the PICS schemas
- `settings/runtime/dataset/formats/*.dip` and `settings/runtime/schemas/dataset_format.dip`
- `settings/runtime/dataset/sources/*.dip` and `settings/runtime/schemas/dataset_source.dip`
- `settings/runtime/themes/*.dip` and `settings/runtime/schemas/appearance.dip`

PICS source pages use DIPL's `?url` metadata on each `pics[...]` entry, rather
than a `source_url` value. If an older local override sets `source_url`, remove
that assignment; each catalog URL is attached to its site entry under
`settings/runtime/pics/`. The directory has four catalog files for CEOS
reference sites, other USGS catalog sites, desert PICS, and reference stations.
Each file is registered in `settings/runtime/DIPfile`.
The six CEOS reference sites have published CEOS and CNES regions. Other USGS
catalog sites use published CNES and SDSU regions, whose catalog centers may
lie outside the regions. A labelled regional or station search window helps
find overpasses but does not define calibration pixels.
The PICS schemas declare latitude, longitude, and region bounds in `deg`,
usable distances in `km`, and altitude in `m`. Values without an explicit unit
in a site entry use the unit declared by its schema.
Use `h5jive --settings-help 'dataset_sources[gome].product_regex'` to read a
setting's DIPL description. The same command accepts other DIPL paths, such as
`pics[algeria_3]`, and shows source URLs where available. The schema also
provides DIPL `?example` metadata for product regexes and scan flag lists.

The `h5jive` appearance profile matches the project's static page: deep
charcoal backgrounds, sage text, and orange/copper accents. It is the
startup default; other profiles remain available from the Appearance menu.
The default data colormap is Viridis. Theme colors and grayscale remain
available in the 2D slice settings.

## Dataset browser

A `dataset_sources[...]` entry registers one local repository in its own DIP
file. Its `root` is the directory to scan, `format` links to a
`dataset_formats[...]` entry, and `product_regex` matches paths relative to
the root. Several sources may use the same format or point to the same
directory with different product rules. Named regex captures `year`, `month`,
`day`, and `orbit` fill the product table. Captures can come from folder names,
file names, or both. For example, a repository organized by date can use
`^(?P<year>[0-9]{4})/(?P<month>[0-9]{2})/(?P<day>[0-9]{2})/` at the start of
its regex. Product metadata can be read on demand when a field is missing.
The `tags` setting is a comma-separated list used by the dataset tag picker
and tag search. The product table has separate searches for name/path, date,
and orbit. Date accepts compact or separated dates and partial prefixes;
orbit accepts a full number or prefix.

Each source declares its own format link, for example
`format link = {?dataset_formats[gome_l1b]}`. The source schema requires the
`format` node; the link target is declared in the source because different
repositories can use different formats.

The bundled source definitions are examples of DIPL settings for supported
product formats. Most use `root = "./"` as a portable default; the source
checkout's ERA5 example points to its bundled sample directory. Put your local
dataset roots in `settings/runtime/overrides.dip`; the app creates this
Git-ignored file on first launch if it is missing. Relative roots are resolved
from the `settings` directory. A native release includes a blank override file
in its `settings` directory; the installer preserves local edits when upgrading.
If an older override uses a `mock_` source ID, remove that prefix. For example,
use `dataset_sources[gome].root` instead of
`dataset_sources[mock_gome].root`.

The browser stores one index per source under
`settings/runtime/.cache/datasets/` and loads a cached index on startup. If no
cache exists, select the source and click **Refresh index** to start a
background scan. **Cancel scan** stops it. Scanning reads directory
entries and file metadata without opening every product. Missing mounts are
marked red while their cached products remain listed. The table supports
searching, sorting, and column resizing. **Open as A** and **Open as B** assign
the selected product to the inspector file slots; the panel below the table
shows loaded, loading, and queued products.

Each source index is a JSON file named after its source ID. A completed refresh
replaces it; closing the app does not delete it. When the configured root or
product regex changes, the old index is ignored until the next refresh. You can
delete `.cache/datasets/` to clear all saved indexes; scans will recreate them.

## Supported dataset formats

H5Jive registers these mission and collection layouts in DIPL. A format entry
describes how measurements relate to geographic coordinates; a source entry
points to a repository of files. Ordinary numeric HDF5 and NetCDF arrays can be
inspected without either entry.

| Format class | How H5Jive locates geography |
| --- | --- |
| [GOME Level 1B](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/gome_l1b.dip) | Band observations with sibling geodata; mode groups identify forward/backward scans |
| [SCIAMACHY Level 1C](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/sciamachy_l1c.dip) | Band observations with sibling geodata and a backscan flag |
| [GOME-2 Level 1B](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/gome2_l1b.dip) | One layout for MetOp-A/B/C; sibling geodata and per-pixel scan flags |
| [FDR4ATMOS UVN](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/fdr_atmos_uvn.dip) | One layout for Level 1A/1B, with GOME and SCIAMACHY groups |
| [Sentinel-5P TROPOMI](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/s5p_tropomi.dip) | Radiance bands use sibling `OBSERVATIONS`/`GEODATA`; Level-2 gas, aerosol, and cloud swaths use `PRODUCT` coordinates |
| [NASA OB.DAAC Ocean Color Level 2](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/obpg_ocean_l2.dip) | MODIS, VIIRS, and PACE OCI swaths with pixel-matched `navigation_data` |
| [VIIRS Dark Target aerosol Level 2](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/viirs_dark_target_l2.dip) | Geophysical observations with sibling `geolocation_data` |
| [ERA5 single levels](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/era5_single_levels.dip) | CF regular grid with `valid_time` |
| [GridMET annual climate grids](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/gridmet.dip) | CF regular grid with `day`, `lat`, and `lon` |
| [NASA Ocean Color Level 3 SMI](https://github.com/vrtulka23/h5jive/blob/main/settings/runtime/dataset/formats/obpg_ocean_l3_smi.dip) | Regular mapped ocean-color grid |

Bundled source entries are examples. You can register several repositories with
the same format, such as separate GOME-1 processing releases, by giving each
source its own ID and root. Grouped products require coordinate arrays that
match the observation pixels. Multi-file packages and subsampled navigation
control points need additional handling before they can be mapped.

When a mission has multiple group arrangements, its format can declare
`layouts[]` entries with a filename glob and the coordinate-group relationship
for each arrangement. TROPOMI uses this for Level-1B radiance and Level-2
retrievals. This is a structural distinction: NO₂, ozone, methane, and other
Level-2 variables share the `PRODUCT` geolocation rule when their coordinate
arrays match the selected observation.

To request support for another layout, [open a GitHub issue](https://github.com/vrtulka23/h5jive/issues/new)
and include a representative file or the product format specification. New
format definitions can then be added to DIPL without duplicating an existing
mission class.

## Supported data

The data-source crate reads HDF5/NetCDF-4 and classic NetCDF. Numeric datasets
can be viewed directly. For map discovery, datasets should reference auxiliary
latitude and longitude coordinates with CF style `coordinates` or descriptive
names/attributes. Latitude and longitude should match the dataset shape, or
share its spatial dimensions.

Pixel footprint bounds should have the spatial dimensions followed by a corner
axis of length four. The renderer interprets each pixel's four corners in
sequence and clips geometry to the map canvas.

For NetCDF, use CF references to make these links explicit: the observation
variable's `coordinates` attribute names its latitude and longitude variables
(for example, `coordinates = "latitude longitude"`), and each coordinate
variable's `bounds` attribute names its corresponding bounds variable. The
viewer follows these references first. It checks coordinate shapes to confirm
they can be indexed with the observation data; shape compatibility is a
validation step, not the link between variables. Conventional coordinate and
bounds names remain a fallback for older files without those attributes.

NetCDF groups can put observations and geolocation in sibling groups. The
bundled GOME, SCIAMACHY, GOME-2, and Sentinel-5P TROPOMI formats use
`OBSERVATIONS` and sibling `GEODATA` groups under each band. Each format has
its own DIPL entry in `settings/runtime/dataset/formats/`. Its `identification`
subnode specifies patterns for root `platform`, `sensor`, and `product_type`
attributes, plus a fallback filename glob. Two matching metadata attributes
identify a format even if the filename is damaged. With fewer available
attributes, the filename provides a fallback. A conflicting attribute rejects
the format. The bundled GOME-2 rule covers MetOp-A/B/C, whose filenames share
a pattern and whose geolocation layout is the same. Format settings are
independent of repository
paths and dates. The viewer checks coordinate shapes before applying a format.
Explicit coordinate paths need no format entry.

The optional `time_coordinate` field names the one-dimensional CF time
variable for an observation's leading dimension. It can differ from the
dimension name: ERA5 uses `valid_time`, while Sentinel-5P radiance uses `time`.
The reader checks that the coordinate uses the observation's first dimension.
When the field is empty or that variable is absent, it looks for a coordinate
named after the first dimension. If no numeric CF time coordinate is available,
combined export uses its configured index dimension instead. The bundled
GOME-2 Band 3 sample is an example: it has a leading time dimension but no
time coordinate values.

Formats can also define a `scan_direction` subnode for a per-pixel flag. Set
`group` to `coordinate`, `observation`, or another sibling group path. Set
`variable` to the flag name, and
`forward_values` and `backward_values` to comma-separated integer codes. The
`default_filter` setting chooses the initial world/globe selection (`forward`,
`backward`, or `both`); bundled GOME-2 and SCIAMACHY formats start on `forward`.
The flag must share the observation's spatial dimensions. GOME-2 uses
`GEODATA/scan_direction` (1 forward, 2 backward); SCIAMACHY uses
`OBSERVATIONS/backscan_flag` (0 forward, 1 backward). The world map and globe
then offer icon buttons for both, forward, and backward scans. They filter the
loaded pixels immediately; the both-directions setting also includes pixels
with unknown flag codes.

When direction is encoded by the surrounding group instead of a pixel flag,
list its ancestor names in `forward_groups` and `backward_groups`. GOME-1 uses
this layout for separate scan-mode groups. The same general fields can describe
other products that divide forward and backward observations into groups.

Four-dimensional radiance arrays use their first three dimensions for map
geolocation and a pinned fourth-axis index for values. The world map and globe
have separate `pinned_default` settings under `viewer.views.world_map` and
`viewer.views.globe`; each accepts `start`, `middle`, or `end`. Their data display
controls apply when **Update** is clicked. Axis names come from NetCDF or HDF5
dimension metadata. Synthetic dimension scales are omitted from value labels.

The bundled GOME-2 Band 3 sample contains radiance channels, pixel centers,
four footprint corners, and scan direction copied from a Level 1B product.
The bundled Sentinel-5P source can index TROPOMI Level 1B radiance bands after
you set its `root` to a local product directory. Their radiance arrays have
time, scanline, ground-pixel, and spectral axes; the sibling `GEODATA` arrays
provide pixel centers and four corners. These products do not contain a
per-pixel forward/backward flag, so the scan-direction controls are unavailable.

For any geolocated dataset with four-corner pixel bounds, the map and globe
combine neighboring source pixels into display cells when the pixel count
exceeds the current display budget. Each cell uses the source block's outer
geographic corners and the mean of its valid values. The corner directions are
derived from nearby pixel centers, so formats can use different corner orders.
Cells containing multiple scan directions are marked mixed and appear when both
directions are shown; zooming in separates them into smaller cells.
As zoom increases, the cells shrink; at full detail, each source pixel is drawn
with its own bounds. Datasets without bounds keep point sampling. Set
`viewer.views.world_map.max_points` in `settings/runtime/viewers/world_map.dip` to
control the whole-world cell count and `zoom_detail_factor` to control how
quickly detail grows with zoom. Both settings apply to map and globe views.
