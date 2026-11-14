# Settings and data formats

## DIPL settings

The editable project is `settings/runtime/DIPfile`. It loads the viewer configuration,
appearance profiles, PICS catalog, and their schemas. User-facing values such as
map zoom, point limits, color ranges, and view defaults live in DIPL. Constraints
that DIPL can express belong in schemas rather than duplicated Rust checks.
The dataset-format schema checks overpass grouping depth, scan direction options,
and the syntax and required presence of scan flag lists. Rust still converts
those lists to integers and checks that forward and backward codes do not overlap.

`settings/build/DIPfile` holds values compiled into the executable. The first
compiled values are Plugin Lab icon rail dimensions. Build settings never read
`runtime/overrides.dip`; release packages contain only the runtime project.

The main files are:

- `settings/runtime/viewer.dip` and `settings/runtime/schemas/viewer.dip`
- `settings/runtime/pics.dip` and the PICS schemas
- `settings/runtime/dataset/formats/*.dip` and `settings/runtime/schemas/dataset_format.dip`
- `settings/runtime/dataset/sources/*.dip` and `settings/runtime/schemas/dataset_source.dip`
- `settings/runtime/themes/*.dip` and `settings/runtime/schemas/appearance.dip`

The `h5jive` appearance profile matches the project's static page: deep
charcoal backgrounds, sage text, and orange/copper accents. It is the
startup default; other profiles remain available from the Appearance menu.
The default data colormap is Viridis. Theme colors and grayscale remain
available in the 2D slice settings.

## Dataset browser

A `dataset_sources[...]` entry registers one local repository in its own DIP
file. Its `root` is the directory to scan, `format` links to a
`dataset_formats[...]` entry, and `product_regex` matches paths relative to
the root. Several sources may use
the same format, and several sources may point at the same directory with
different product rules. Named regex captures `year`, `month`, `day`, and
`orbit` fill the product table. Captures can come from folder names, file names,
or both. For example, a repository organized by date can use
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

The bundled mock registrations use `root = "./"` as a portable default. Put
your local dataset roots in `settings/runtime/overrides.dip`; the app creates this
Git-ignored file on first launch if it is missing. Relative roots are resolved
from the `settings` directory. A native
release includes a blank override file beside its executable; the user
installer preserves local edits when upgrading. The browser stores one index
per source under `settings/runtime/.cache/datasets/` and loads a cached index on
startup. If no cache exists, select the source and click **Refresh index** to
start a background scan. **Cancel scan** stops it. Scanning reads directory
entries and file metadata;
it does not open every product. Missing mounts are marked red while their
cached products remain listed. The table supports searching, sorting, and
column resizing. **Open as A/B** assigns the selected product to the inspector
file slots; the panel below the table shows loaded, loading, and queued products.

The browser can check products against individual PICS regions on demand. Choose
an **Overpass area** and click **Check overpasses** for the selected repository.
The background check reads only latitude and longitude coordinate slabs, not
observation values, and stops reading a group once a pixel center enters the
area. It uses at most two reader threads to limit disk contention, reports
progress, and can be cancelled. Completed checks are saved under
`settings/runtime/.cache/datasets/`, which Git ignores, and reused by **Check
overpasses** when file size and modification time are unchanged. Selecting an
area restores saved matches for products with the indexed size without opening
them; **Refresh overpasses** validates their modification times. The table shows matching products after a check;
**Show all products** removes that filter. Products with multiple geolocation
groups have a group selector, so FDR can be filtered by GOME or SCIAMACHY. This is a
pixel-center test, so a footprint touching an area without its center entering
it does not count as an overpass.

The `overpass_group_depth` format setting controls how hits within one product
are separated. Zero gives one result per product. FDR uses depth 1 so its
top-level GOME and SCIAMACHY groups are reported independently. Other formats
can use the same rule for any grouped product layout.

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
bundled GOME, SCIAMACHY, GOME-2, Sentinel-5P TROPOMI, and FDR4ATMOS formats use `OBSERVATIONS`
and sibling `GEODATA` groups under each band. Each format has its own DIPL
entry in `settings/runtime/dataset/formats/`. Its `identification` subnode specifies
patterns for root `platform`, `sensor`, and `product_type` attributes, plus a
fallback filename glob. Two matching metadata attributes identify a format
even if the filename is damaged. With fewer available attributes, the filename
provides a fallback. A conflicting attribute rejects the format. The bundled
GOME-2 rule covers MetOp-A/B/C, whose filenames share a pattern and whose
geolocation layout is the same. Format settings are independent of repository
paths and dates. The viewer checks coordinate shapes before applying a format.
Explicit coordinate paths need no format entry.

Formats can also define a `scan_direction` subnode for a per-pixel flag. Set
`group` to `coordinate` or `observation`, `variable` to the flag name, and
`forward_values` and `backward_values` to comma-separated integer codes. The
`default_filter` setting chooses the initial world/globe selection (`forward`,
`backward`, or `both`); bundled GOME-2 and SCIAMACHY formats start on `forward`.
The flag must share the observation's spatial dimensions. GOME-2 uses
`GEODATA/scan_direction` (1 forward, 2 backward); SCIAMACHY uses
`OBSERVATIONS/backscan_flag` (0 forward, 1 backward). The world and globe views
then offer **Both**, **Fwd**, and **Back** buttons. They filter the loaded pixels
immediately; **Both** also includes pixels with unknown flag codes.

Four-dimensional radiance arrays use their first three dimensions for map
geolocation and a pinned fourth-axis index for values. The world map and globe
have separate `pinned_default` settings under `viewer.views.world_map` and
`viewer.views.globe`; each accepts `start`, `middle`, or `end`. Their data display
controls apply when **Update** is clicked. Axis names come from NetCDF or HDF5
dimension metadata. Synthetic dimension scales are omitted from value labels.

The bundled GOME-2 Band 3 sample contains radiance channels, pixel centers,
four footprint corners, and scan direction copied from a Level 1B product.
The local Sentinel-5P source indexes TROPOMI Level 1B radiance bands under
`../mock_datasets/S5P`. Their radiance arrays have time, scanline, ground-pixel,
and spectral axes; the sibling `GEODATA` arrays provide pixel centers and four
corners. These products do not contain a per-pixel forward/backward flag, so the
scan-direction controls are unavailable.

For any geolocated dataset with four-corner pixel bounds, the map and globe
combine neighboring source pixels into display cells when the pixel count
exceeds the current display budget. Each cell uses the source block's outer
geographic corners and the mean of its valid values. The corner directions are
derived from nearby pixel centers, so formats can use different corner orders.
Cells containing multiple scan directions are marked mixed and appear in Both
mode; zooming in separates them into smaller cells.
As zoom increases, the cells shrink; at full detail, each source pixel is drawn
with its own bounds. Datasets without bounds keep point sampling. Set
`viewer.views.world_map.max_points` in `settings/runtime/viewers/world_map.dip` to
control the whole-world cell count and `zoom_detail_factor` to control how
quickly detail grows with zoom. Both settings apply to map and globe views.
