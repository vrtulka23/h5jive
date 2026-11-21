# Plugin Lab catalog

This catalog lists the plugins compiled into H5Jive. Contributions are
reviewed as pull requests; using a new plugin means building a version of
H5Jive that contains its source.

| Plugin | Purpose | Setting |
| --- | --- | --- |
| [A/B path comparison](../crates/h5jive/src/plugins/path_comparison.rs) | Compare paths and metadata, then copy missing groups or datasets in either direction without replacing existing paths | `settings/runtime/plugins.dip`: `path_comparison_enabled` |
| [Dataset value comparison](../crates/h5jive/src/plugins/value_comparison.rs) | Pick matching numeric datasets from A or B and view their difference or ratio in 2D | `settings/runtime/plugins.dip`: `value_comparison_enabled` |
| [Dataset manipulation](../crates/h5jive/src/plugins/dataset_manipulation.rs) | Create a new dataset by slicing one numeric dataset or adding, subtracting, multiplying, or dividing two matching datasets | `settings/runtime/plugins.dip`: `dataset_manipulation_enabled` |
| [Spatiotemporal subset](../crates/h5jive/src/plugins/spatiotemporal_subset.rs) | Preview combined geographic coverage and export selected products, variables, and dates as native groups or a gridded NetCDF-4 mosaic | `settings/runtime/plugins.dip`: `spatiotemporal_subset_enabled` |

To contribute, follow the [Plugin Lab guide](../docs/src/plugin-lab.md), add
your plugin to the registry, give it a dedicated DIPL settings file and demo,
and open a pull request. Add one row here describing the tool and its switch.
