# H5Jive Guide

H5Jive is a native desktop application for exploring scientific datasets.
It combines a navigable file tree, dataset details, and linked views for tables,
profiles, 2D slices, 3D volumes, and geolocated data.

Use this guide to install and run the application, inspect datasets, configure
the viewer, or work on the Rust codebase. For project overview and current
capabilities, see the [root README](../../README.md). For generated Rust API
documentation, run `cargo doc --workspace --open`.

## At a glance

- Open HDF5 and classic NetCDF files.
- Inspect dataset shape, type, statistics, attributes, and coordinate axes.
- Explore numeric arrays as tables, profiles, 2D slices, and 3D voxel previews.
- Map datasets with CF style latitude and longitude coordinates, including
  pixel footprints when coordinate bounds are available.
- Customize the interface and view defaults through DIPL settings.
