# Plugin Lab catalog

This is the small source based app store for H5Jive plugins. Plugins are
reviewed as pull requests and compiled into the app; installing a plugin means
building a version of H5Jive that contains its source.

| Plugin | Purpose | Setting |
| --- | --- | --- |
| [A/B path comparison](../crates/h5jive/src/plugins/path_comparison.rs) | Compare paths and metadata, then copy missing groups or datasets in either direction without replacing existing paths | `settings/runtime/plugins.dip`: `path_comparison_enabled` |
| [Dataset value comparison](../crates/h5jive/src/plugins/value_comparison.rs) | Pick matching numeric datasets from A or B and view their difference or ratio in 2D | `settings/runtime/plugins.dip`: `value_comparison_enabled` |

To contribute, follow the [Plugin Lab guide](../docs/src/plugin-lab.md), add
your plugin to the registry, give it a dedicated DIPL settings file and demo,
and open a pull request. Add one row here describing the tool and its switch.
