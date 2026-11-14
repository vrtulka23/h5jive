# Settings conventions

`build/` is evaluated by `crates/h5jive/build.rs`; `runtime/` is loaded by
`app/settings.rs`. Keep each schema, source, and Rust consumer in agreement.

1. `runtime/DIPfile` registers schemas and named sources, includes the viewer,
   dataset formats and sources, and themes, then applies `overrides.dip`.
   Register new files there; do not rely on a file merely existing on disk.
2. Define runtime defaults, types, descriptions, and validation in `runtime/schemas/`.
   Use the established `name type = default` syntax for leaf values and add
   `!condition`, `!options`, and `!setup` where they clarify valid choices.
   Keep source values typed as well; an untyped new leaf can fail while DIPL
   loads a named source.
3. Keep the runtime instance hierarchy in `runtime/viewer.dip`. View options
   live in `runtime/schemas/views/` and `runtime/viewers/`. Theme values live
   under `runtime/themes/` and follow `runtime/schemas/appearance.dip`.
4. Put each Plugin Lab plugin's runtime options in `runtime/schemas/plugins/`
   and `runtime/plugins/`. Put its typed `<plugin>_enabled bool` switch in
   `runtime/plugins.dip`, declare it in `runtime/schemas/viewer.dip`, and wire
   the plugin source under `runtime/viewer.dip`. Read new values in
   `crates/h5jive/src/app/settings.rs` and connect the switch to the plugin
   registry.
5. Treat `runtime/overrides.dip` as a local user override, not a place for project
   defaults. Preserve existing setting names and defaults unless a migration
   is intentional, since users may already have overrides.
6. Keep `build/DIPfile` independent of local overrides. Its compiled fields
   are consumed through generated Rust code in `OUT_DIR`. Do not duplicate a
   parameter between build and runtime projects without a documented reason.
7. Validate a change by loading the project without starting the GUI, for
   example `cargo run -p h5jive -- --inspect examples/plugins/path_comparison/plugin_demo_a.hdf5`.
   Run relevant Rust tests; the full suite uses
   `cargo test -p h5jive -- --test-threads=1` because DIPL tests share state.
