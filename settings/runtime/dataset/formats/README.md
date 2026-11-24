# Dataset formats

Each `.dip` file defines one `dataset_formats[...]` entry. A format identifies
products and describes observation and coordinate layout, bounds, optional
`time_coordinate`, and scan direction flags. The schema is
[`schemas/dataset_format.dip`](../../schemas/dataset_format.dip).

The parent [dataset metadata](../README.md) links these formats to repository
sources. Register a new format in the [runtime DIPfile](../../DIPfile); a file
that is merely present here is not loaded. An empty `time_coordinate` keeps
automatic first-dimension coordinate discovery.
