# DIPL projects

- `build/DIPfile` defines settings compiled into the executable by
  `crates/h5jive/build.rs`. Editing these requires `cargo build`. Its
  `resource_limits.dip` sets ceilings for the file tree, statistics sample,
  2D slice sample, and 3D preview sample.
- `runtime/DIPfile` defines editable application settings. Its
  `overrides.dip` is created on first launch and is never read during builds.
  The corresponding runtime budgets can be lowered or raised up to their
  compiled ceilings; an override above a ceiling prevents startup and names
  the offending setting.

Release packages copy `runtime/` to `settings/` beside the executable (or to
the platform's application resources). `build/` is used only while compiling.
