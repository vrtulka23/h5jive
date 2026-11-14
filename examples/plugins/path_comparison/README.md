# A/B path comparison demo

Generate two small HDF5 files from the repository root:

```sh
cargo run -p h5jive --example generate_plugin_demo
cargo run -p h5jive -- examples/plugins/path_comparison/plugin_demo_a.hdf5 examples/plugins/path_comparison/plugin_demo_b.hdf5
```

Open **Plugin Lab → A/B path comparison**. The files include matching paths,
shape and datatype changes, changed and missing attributes, a group whose
attributes changed, objects unique to each side, and a path that changes from
a group to a dataset. `value_only` has different numeric values but matching
metadata. Double-click a dataset in either column to open it in the core Data inspector.
From there, use the existing visualization selector for compatible datasets.
Use the status count buttons to filter the list. Clicking a path in either
column reveals its details in both columns; both columns scroll together.

To try synchronization, make copies of the demo files first. Use **Copy missing
A → B** or **Copy missing B → A**, review the proposed count, and confirm. The
missing-path status icon can copy one path. The destination is reloaded when
copying finishes; existing paths are not replaced.
Click a **Different** status icon to replace an existing object in either
direction. This also replaces dataset values or group children, so review the
confirmation prompt first.

The comparison checks paths, object kinds, dataset shapes and datatypes, and
group and dataset attribute names and displayed values. It does not compare
dataset values. Large or unsupported attribute values can be summarized by the
metadata reader. Metadata is scanned in the background so large files do not
freeze the interface.
