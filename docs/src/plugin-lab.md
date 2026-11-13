# Plugin Lab

Plugin Lab is H5Jive's collection of optional, source-level extensions. Its
first plugin compares paths in the loaded files A and B. It labels a path
**Same** when object kind, dataset shape and datatype, and displayed attribute
values match, **Different** when they do not, and **Only A/B** when the path is
absent from one file. Dataset values are not compared. Attribute comparison uses
the values exposed by H5Jive's metadata reader; large or unsupported attribute
values may be summarized. Double-click a dataset in either column to open it in the core
Data inspector; compatible datasets can then use the other core views.

## Try the demo

From the repository root:

```sh
cargo run -p h5jive --example generate_plugin_demo
cargo run -p h5jive -- examples/plugin_demo_a.hdf5 examples/plugin_demo_b.hdf5
```

Open **Plugin Lab** in the top mode bar. Both files are loaded through H5Jive's
normal file slots, so plugins use the same file trees and dataset access as the
inspector. Choose a plugin using its icon in the vertical tab rail on the left.
The A and B panels share one scrollable row grid, keeping matching paths aligned
as you scroll. Status icons indicate a match, a metadata difference, a path
present on only one side, or metadata that could not be read. Hover an icon for
the fields that differ. Use the count buttons to filter paths by status. Path
details start collapsed; click a path or chevron on either side to reveal metadata on both sides.
The metadata scan runs in the background.

## Add a plugin

Coding agents should also read the [plugin agent notes](https://github.com/vrtulka23/h5jive/blob/main/crates/h5jive/src/plugins/AGENTS.md)
before editing the plugin registry or adding a module.

1. Download or clone [the source code](https://github.com/vrtulka23/h5jive).
2. Create a module under `crates/h5jive/src/plugins/`. Read data through
   `PluginContext.files`, inspect `PluginContext.selection` when needed, and
   return a `Selection` to open a dataset in the core
   inspector. Keep plugin UI and analysis state in the module.
3. Register it in `crates/h5jive/src/plugins/mod.rs` with a unique key, icon,
   title, enabled setting, and draw function. The icon appears in the left tab
   rail when the plugin is enabled.
4. Give it a dedicated schema in `settings/schemas/plugins/` and a settings
   file in `settings/plugins/`. Add its `enabled` switch to the central
   `settings/plugins.dip` and wire the values through `Settings`.
5. Add a reproducible demo in `examples/`, run `cargo test -p h5jive`, and
   submit a pull request with a short description and screenshot. The Plugin
   [Plugin Lab catalog](https://github.com/vrtulka23/h5jive/tree/main/plugins)
   is the project's small app store for shared source plugins.

The current design compiles plugins with H5Jive. It is a source extension
point, not a runtime loader for external binaries.

### A prompt for an AI coding agent

If you are a scientist new to Rust, start with a precise task and a small
sample file. For example:

> In H5Jive's Plugin Lab, add a plugin that computes the mean of the selected
> numeric HDF5 dataset and shows the result beside its path. Use the existing
> file and bounded region APIs; keep the code in `src/plugins`. Add a separate
> DIPL schema and settings file, a central enabled switch, a tiny example in
> `examples/`, and a test. Explain how to run the demo.

Check the output on a dataset whose mean you know, and review the code before
submitting it.
