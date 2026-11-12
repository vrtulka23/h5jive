# Architecture

## Workspace

- `crates/data-source` handles file access, dataset discovery, attributes,
  statistics, coordinate discovery, and bounded region reads.
- `crates/h5jive/src/core/` contains GUI-independent dataset indexing,
  slice models, value classification, and profile, heatmap, volume, and
  geospatial loading routines.
- `crates/h5jive/src/app/` contains the native UI, view controls,
  rendering, settings, and background job coordination. Views call the core
  routines to obtain data for display.
- `settings/` contains DIPL configuration, schemas, and appearance profiles.
- `examples/` contains the generated array demo and a compact GOME-2 Band 3 sample.
- `website/` is the static Astro project page; `docs/` is this mdBook guide.

## Build and API reference

```sh
cargo check --workspace
cargo doc --workspace --no-deps
./h5jive.sh --build
```

Run the reproducible demo generator with:

```sh
cargo run -p h5jive --example generate_demo -- examples/space_demo.hdf5
```

The Band 3 sample is regenerated from a local Level 1B product with:

```sh
cargo run -p h5jive --example generate_gome2_band3 -- \
  ../mock_datasets/GOME2/01/06/20130106101404Z_32260_gome.nc \
  examples/gome2_band3_20130106_gome.nc
```

## Performance checks

The ignored release-mode tests measure product filtering with 70,000 rows and
world-map loading from a local GOME-2 mock product:

```sh
cargo test -p h5jive --release benchmark_ -- --ignored --nocapture --test-threads=1
```

The map benchmark skips itself if `../mock_datasets/GOME2/01/12/20130112080756Z_32344_gome.nc`
is unavailable. Timings depend on the filesystem cache and storage device.
