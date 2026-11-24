# Spatiotemporal subset demo

Generate three overlapping NetCDF swaths with `u10` and `v10`:

```sh
cargo run -p h5jive-data --example generate_subset_demo
```

Register `examples/plugins/spatiotemporal_subset/products` as a Dataset Browser
source. For local development, save this as
`settings/runtime/dataset/sources/subset_demo.dip` (or adapt its root to your
repository), and add `code[]` with that file path to `settings/runtime/DIPfile`:

```dip
dataset_sources[subset_demo] : dataset_source
  name = "North Sea subset demo"
  root = "../../examples/plugins/spatiotemporal_subset/products"
  format link = {?dataset_formats[s5p_tropomi]}
  tags = "demo, wind"
  product_regex = "^north_sea_(?P<year>[0-9]{4})(?P<month>[0-9]{2})(?P<day>[0-9]{2})_orbit(?P<orbit>[0-9]+)[.]nc$"
```

In Dataset Browser, index the source. In Plugin Lab, choose **Spatiotemporal
subset**, select the source, and set dates `2024-01-01` through `2024-01-03`.
Select `u10` and `v10`, open the combined coverage popup, then enter west `1`,
south `53`, east `5`, north `57`.
Export a **Gridded mosaic** to see one mean grid and per-cell sample counts,
or **Native products** to keep the three cropped swaths in separate groups.
For a collection of compatible grids or swaths, choose **Combined time series**
to write one array with a shared leading dimension. Existing CF time values are
placed in chronological order. For grids without a time coordinate, set the
first dimension name (default `product`); its values are sample indexes. Set an
optional group name to nest the series or leave it empty to write at the root.
Swaths keep coordinates, bounds, and scan direction per sample. Unequal scan
lengths are padded with missing values; `row_count` and `column_count` give the
valid size of each sample. The export rejects different regular grids, units,
calendars, and overlapping timestamps.
The mosaic method selector also offers Newest, Oldest, Minimum, and Maximum.
Newest and Oldest use the indexed product order and CF time within a product;
`_count` always reports all valid samples considered for each cell.
The output is a new NetCDF-4 `.nc` file. Existing files are protected.

This first version supports numeric 2D swaths and regular latitude/longitude
grids with an optional leading CF numeric time dimension. Mosaic export combines
selected products and time samples using the chosen method; native export keeps
the time axis. Native export also keeps all channels in 4D geolocated fields such
as GOME-2 radiance. Mosaic export combines each channel separately.
Other coordinate geometries need a later extension.

## Real ERA5 sample

[`era5/ERA5_2026_04.nc`](era5/ERA5_2026_04.nc) is a 588 KiB excerpt of ERA5
single-level data hosted by [ERA5WindRecon](https://github.com/DLWangSan/ERA5WindRecon/blob/main/example_data/ERA5_2026_04.nc).
It contains 192 hourly samples from 1–8 April 2026 on a 0.25° grid spanning
20–25° N, 113–117° E, with `u10`, `v10`, `t2m`, and `msl`. This is a real ERA5
sample near southern China; the North Sea data above are synthetic. See the
[ERA5 data licence and attribution](era5/LICENSE.md) before redistributing it.

The source checkout registers the ERA5 sample automatically. Index **ERA5
example** in Dataset Browser. Installed packages do not bundle this example;
set `dataset_sources[era5_example].root` in `settings/overrides.dip` to a local
ERA5 directory when using an installed copy. Adjust its `product_regex` if
your filenames use a different convention.

In Spatiotemporal subset, choose dates
`2026-04-02` through `2026-04-08`, select `u10` and `v10`, and set bounds west
`114`, south `21`, east `116`, north `24`. Export **Native products** to keep
all 168 selected hourly samples. The output contains 13 latitude cells and 9
longitude cells, with both wind variables in a `product_0000` group. The sample
is small enough for a quick check of discovery, time cropping, and NetCDF export;
it does not measure performance on a full year of global ERA5 data.
