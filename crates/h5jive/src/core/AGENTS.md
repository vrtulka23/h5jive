# Core conventions

This directory holds dataset selection models, bounded data loading,
value classification, and visualization data preparation without GUI code.
`mod.rs` reexports the crate-visible pieces used by the app and plugins.

1. Keep this layer independent of Dear ImGui, Dear ImPlot, wgpu, and window
   state. Return data, keys, or errors that callers can render or explain.
   Use `h5jive-data` for HDF5 and NetCDF access instead of opening backend
   libraries directly here.
2. Preserve arbitrary dataset shape and axis order. Check shapes, ranges,
   indexing, and overflow before allocating or reading. Follow existing
   region and slice models so large datasets are read in bounded chunks.
3. Keep dataset identity explicit in cache keys, including file slot, path,
   slice or region parameters, and resolution where applicable. Avoid stale
   results after a file, dataset, or view setting changes.
4. Distinguish valid finite values from NaN, infinities, and fill values using
   the existing value visibility and classification helpers. Keep their
   behavior consistent across tables, profiles, heatmaps, volumes, and maps.
5. Add focused tests for calculations, edge shapes, and boundary cases.
   Run `cargo fmt --all`, `cargo check -p h5jive`, and the relevant tests;
   use `cargo test -p h5jive -- --test-threads=1` for the full suite.
