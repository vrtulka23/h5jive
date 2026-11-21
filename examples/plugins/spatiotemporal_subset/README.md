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
  format link = {?dataset_formats[s5p_tropomi_l1b_radiance]}
  tags = "demo, wind"
  product_regex = "^north_sea_(?P<year>[0-9]{4})(?P<month>[0-9]{2})(?P<day>[0-9]{2})_orbit(?P<orbit>[0-9]+)[.]nc$"
```

In Dataset Browser, index the source. In Plugin Lab, choose **Spatiotemporal
subset**, select the source, and set dates `2024-01-01` through `2024-01-03`.
Select `u10` and `v10`, preview the combined coverage, then drag a rectangle
around the North Sea or enter west `1`, south `53`, east `5`, north `57`.
Export a **Gridded mosaic** to see one mean grid and per-cell sample counts,
or **Native products** to keep the three cropped swaths in separate groups.
The output is a new NetCDF-4 `.nc` file. Existing files are protected.

This first version supports numeric 2D swaths and regular latitude/longitude
grids with an optional leading CF numeric time dimension. Mosaic export is a
mean across selected products and time samples; native export keeps the time
axis. Other coordinate geometries need a later extension.
