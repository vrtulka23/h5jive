# Build settings

This independent DIPL project defines constants generated during compilation.
[DIPfile](DIPfile) registers the [schemas](schemas/README.md) and the concrete
`plugin_layout.dip` and `resource_limits.dip` values. The Rust build script at
[`crates/h5jive/build.rs`](../../crates/h5jive/build.rs) reads this project and
generates Rust constants in `OUT_DIR`.

These values are not user overrides. Editable application settings belong in
the [runtime project](../runtime/README.md).
