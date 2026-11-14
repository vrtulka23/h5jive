# DIPL projects

- `build/DIPfile` defines settings compiled into the executable by
  `crates/h5jive/build.rs`. Editing these requires `cargo build`.
- `runtime/DIPfile` defines editable application settings. Its
  `overrides.dip` is created on first launch and is never read during builds.

Release packages copy `runtime/` to `settings/` beside the executable (or to
the platform's application resources). `build/` is used only while compiling.
