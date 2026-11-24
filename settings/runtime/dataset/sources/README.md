# Dataset sources

Each `.dip` file defines one `dataset_sources[...]` entry with a local root,
product regex, tags, and a link to a [dataset format](../formats/README.md).
The schema is [`schemas/dataset_source.dip`](../../schemas/dataset_source.dip).

The Dataset Browser indexes products from these roots. Register a new source
in the [runtime DIPfile](../../DIPfile); users can change an existing root in
`settings/runtime/overrides.dip` without changing project defaults.
