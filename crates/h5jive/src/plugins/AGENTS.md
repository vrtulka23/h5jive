# Building a Plugin Lab plugin

These notes apply to new modules in this directory. Read
`docs/src/plugin-lab.md` for the contributor workflow and
`path_comparison.rs` and `value_comparison.rs` for working examples. Keep
each plugin's selection, worker, and preview state independent of inspector
state. Share stateless data and drawing helpers when behavior overlaps.

1. Keep plugin analysis, UI, and state in its own module here. Register the
   plugin in `mod.rs` through `PLUGINS`, with a unique key, title, description,
   enabled setting, icon drawing function, and content drawing function. The
   registry creates its icon tab in the left rail automatically. Add state to
   `PluginLab` only when the plugin needs it. Invalidate cached results when
   files A or B change.
2. Use `PluginContext.files` for the loaded A/B files and their `tree` entries.
   Use `PluginContext.selection` when the current core selection matters. Reuse
   `h5jive-data` and `core` APIs for data access and analysis. For expensive
   metadata scans, use `PluginContext.file_paths` to reopen files on a worker
   and cache the result; invalidate it when A or B changes. Return a
   `Selection` from the plugin to open a dataset in the Data inspector, where
   the core visualization views remain available.
3. Add a central `<plugin>_enabled bool` switch in `settings/runtime/plugins.dip` and
   its declaration in `settings/runtime/schemas/viewer.dip`. Put plugin-specific
   options in `settings/runtime/plugins/<plugin>.dip` with a matching schema under
   `settings/runtime/schemas/plugins/`. Register both files in `settings/runtime/DIPfile`,
   include the plugin settings in `settings/runtime/viewer.dip`, and read them through
   `app/settings.rs`. DIPL source files need explicit types for leaf values.
4. Add a tiny reproducible fixture or generator in `examples/` and document
   how to run it. Test classifications or calculations with a meaningful
   expected result. Run `cargo fmt --all`, `cargo check -p h5jive`, and
   `cargo test -p h5jive -- --test-threads=1`. The serial test run avoids
   shared DIPL and Dear ImGui test state conflicts.
5. Update `plugins/README.md` so the catalog lists the new plugin and its
   switch. Keep claims about value comparison or visualization precise.

Plugins are compiled from source with H5Jive. This directory is not a loader
for untrusted runtime binaries.
