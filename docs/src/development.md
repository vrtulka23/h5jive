# Architecture

## Workspace

- `crates/data-source` (`h5jive-data`) is the backend boundary and handles file
  access, dataset discovery, attributes, mutations, copies, statistics,
  coordinate discovery, and bounded region reads. It is the only crate that
  may depend on HDF5, NetCDF, or ndarray.
- `crates/h5jive/src/core/` contains GUI-independent dataset indexing,
  slice models, value classification, and profile, heatmap, volume, and
  geospatial loading routines. It may use `h5jive-data`, but not the UI or
  plugin layers. Scanners accept cancellation and progress callbacks.
- `crates/h5jive/src/app/` contains the native UI, view controls,
  rendering, settings, and background job coordination. Views call the core
  routines to obtain data for display. All long-running work is polled through
  `BackgroundJob` or `ProgressJob`; do not add raw channel polling. The `App`
  shell delegates state to cohesive groups in `app/view_state.rs`: session,
  workspace, dataset inspector, 2D heatmap/slice, profile, volume, and
  geospatial state. Add fields to the owning group rather than to `App`.
- `crates/h5jive/src/plugins/` contains optional Plugin Lab tools. See
  [Plugin Lab](plugin-lab.md) for the extension contract and demo. Plugins use
  `h5jive-data` for format-specific work and the shared job and notice APIs for
  transient state.

- `settings/build/` contains compiled DIPL constants; `settings/runtime/`
  contains editable DIPL configuration, schemas, and appearance profiles.
- `examples/arrays/`, `examples/satellite/`, and `examples/plugins/` keep the
  general, geospatial, and Plugin Lab fixtures separate.
- `website/` is the static Astro project page; `docs/` is this mdBook guide.

General explanations for the active browser, inspector view, or plugin belong
in **Help → Help for current section**, which opens a popup. Keep controls and
viewers focused on the task; show loading, errors, and actionable empty states
where they occur.

## Build and API reference

```sh
./scripts/check-layering.sh
cargo check --workspace
cargo doc --workspace --no-deps
./h5jive.sh --build
```

`check-layering.sh` verifies the crate dependency directions and rejects new
backend imports in the UI or raw channel polling outside the shared job layer.

When the `snt` CLI with `dip format` is on `PATH`, the H5Jive build script
checks and, when needed, formats the build and runtime settings (including
`DIPfile` and local overrides) before generating compiled settings. Builds
without that formatter skip this step.

Run the reproducible demo generator with:

```sh
cargo run -p h5jive --example generate_demo -- examples/arrays/space_demo.hdf5
```

The Band 3 sample is regenerated from a local Level 1B product with:

```sh
cargo run -p h5jive --example generate_gome2_band3 -- \
  ../mock_datasets/GOME2/01/06/20130106101404Z_32260_gome.nc \
  examples/satellite/gome2_band3_20130106_gome.nc
```

## Performance checks

The ignored release-mode tests measure product filtering with 70,000 rows and
world-map loading from local GOME-2 and S5P products:

```sh
cargo test -p h5jive --release benchmark_ -- --ignored --nocapture --test-threads=1
```

The map benchmarks skip themselves when their files in `../mock_datasets/`
are unavailable. The S5P preview benchmark compares the sampled first view,
radiance-plane decoding, and map rebuilding from a decoded plane. Timings
depend on the filesystem cache and storage device.
