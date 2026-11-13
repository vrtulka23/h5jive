# Settings conventions

This directory is the DIPL project loaded by `app/settings.rs`. Keep the
schema, source files, and Rust settings reader in agreement when changing a
setting.

1. `DIPfile` registers schemas and named sources, includes the viewer,
   dataset formats and sources, and themes, then applies `overrides.dip`.
   Register new files there; do not rely on a file merely existing on disk.
2. Define defaults, types, descriptions, and validation in `schemas/`.
   Use the established `name type = default` syntax for leaf values and add
   `!condition`, `!options`, and `!setup` where they clarify valid choices.
   Keep source values typed as well; an untyped new leaf can fail while DIPL
   loads a named source.
3. Keep the instance hierarchy in `viewer.dip`. View options live in their
   own `schemas/views/<view>.dip` and `viewers/<view>.dip`; wire each source
   into the matching `viewer.views` node. Theme values live under `themes/`
   and follow `schemas/appearance.dip`.
4. Put each Plugin Lab plugin's options in `schemas/plugins/<plugin>.dip` and
   `plugins/<plugin>.dip`. Put its typed `<plugin>_enabled bool` switch in the
   central `plugins.dip`, declare it in `schemas/viewer.dip`, and wire the
   plugin source under `viewer.plugins` in `viewer.dip`. Read new values in
   `crates/h5jive/src/app/settings.rs` and connect the switch to the plugin
   registry.
5. Treat `overrides.dip` as a local user override, not a place for project
   defaults. Preserve existing setting names and defaults unless a migration
   is intentional, since users may already have overrides.
6. Validate a change by loading the project without starting the GUI, for
   example `cargo run -p h5jive -- --inspect examples/plugin_demo_a.hdf5`.
   Run relevant Rust tests; the full suite uses
   `cargo test -p h5jive -- --test-threads=1` because DIPL tests share state.
