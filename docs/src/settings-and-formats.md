# Settings and data formats

## DIPL settings

The root project is `settings/DIPfile`. It loads the viewer configuration,
appearance profiles, PICS catalog, and their schemas. User-facing values such as
map zoom, point limits, color ranges, and view defaults live in DIPL. Constraints
that DIPL can express belong in schemas rather than duplicated Rust checks.
The dataset-format schema checks overpass grouping depth, scan direction options,
and the syntax and required presence of scan flag lists. Rust still converts
those lists to integers and checks that forward and backward codes do not overlap.

The main files are:

- `settings/viewer.dip` and `settings/schemas/viewer.dip`
- `settings/pics.dip` and the PICS schemas
- `settings/dataset/formats/*.dip` and `settings/schemas/dataset_format.dip`
- `settings/dataset/sources/*.dip` and `settings/schemas/dataset_source.dip`
- `settings/themes/*.dip` and `settings/schemas/appearance.dip`

The `h5jive` appearance profile matches the project's static page: deep
charcoal backgrounds, sage text, and orange/copper accents. It is the
startup default; other profiles remain available from the Appearance menu.
The default data colormap uses the profile's blue, cyan, and yellow stops;
Viridis and grayscale remain available in the 2D slice settings.

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

The bundled mock registrations use `root = "./"` as a portable default. The
paths to `../mock_datasets` are set in `settings/overrides.dip`; edit those
assignments for another host. Relative roots are resolved from the `settings`
directory. A native release includes a blank override file beside its
executable; the user installer preserves local edits when upgrading. The
browser scans registered roots in background workers and stores
one index per source under `settings/.cache/datasets/`. It loads a cached index
on startup, scans automatically when no cache exists, and offers **Refresh
index** and **Cancel scan**. Scanning reads directory entries and file metadata;
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
`settings/.cache/datasets/`, which Git ignores, and reused by **Check
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
bundled GOME, SCIAMACHY, GOME-2, and FDR4ATMOS formats use `OBSERVATIONS`
and sibling `GEODATA` groups under each band. Each format has its own DIPL
entry in `settings/dataset/formats/`. Its `identification` subnode specifies
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

The bundled GOME-2 demo contains measured SO₂ slant-column retrievals, pixel
centers, and four footprint corners copied from an orbit extract.
