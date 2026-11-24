# Dataset formats

Each `.dip` file defines one `dataset_formats[...]` mission or collection class.
A format describes observation and coordinate layout, bounds, optional
`time_coordinate`, and scan direction flags or group names. A class with more
than one group arrangement can declare `layouts[]` entries, each selected by a
filename glob. The schema is
[`schemas/dataset_format.dip`](../../schemas/dataset_format.dip).

The parent [dataset metadata](../README.md) links these formats to repository
sources. Register a new format in the [runtime DIPfile](../../DIPfile); a file
that is merely present here is not loaded. An empty `time_coordinate` keeps
automatic first-dimension coordinate discovery.

The registered classes cover these additional structures:

| Format | Observation group | Coordinate group | Scope |
| --- | --- | --- | --- |
| [NASA OB.DAAC Ocean Color](obpg_ocean_l2.dip) | `geophysical_data` | `navigation_data` | MODIS, VIIRS, and PACE OCI swaths with pixel-matched navigation arrays |
| [VIIRS Dark Target aerosol](viirs_dark_target_l2.dip) | `geophysical_data` | `geolocation_data` | NASA AERDT Level-2 swaths |
| [Sentinel-5P TROPOMI](s5p_tropomi.dip) | `OBSERVATIONS` or `PRODUCT` | `GEODATA` or `PRODUCT` | One mission class with radiance and Level-2 gas, aerosol, and cloud layouts |
| [GridMET](gridmet.dip) | root | root | Annual CF grids with `day`, `lat`, and `lon` |
| [NASA Ocean Color Level 3 SMI](obpg_ocean_l3_smi.dip) | root | root | Regular mapped ocean-color fields |

Root-level CF grids use the standard coordinate discovery. The format entries
record their axes and time names for the Dataset Browser and subset selection.
The GOME Level-1B rule also describes forward and backward scans by ancestor
group name; any format may use the same schema fields or a pixel flag in a
sibling group.

These rules use the product filename when the file does not carry matching root
identity attributes. They apply to numeric observation arrays whose latitude
and longitude shapes match the displayed pixels. OB.DAAC products with
subsampled navigation control points need interpolation before geospatial
display. Sentinel-5P NO2 corner bounds live in a nested support group and are
used only when the product's coordinate metadata resolves them. A multi-file
package such as Sentinel-3 OLCI needs cross-file geolocation support before a
format entry can make its measurements geographic.
