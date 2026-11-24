# Dataset sources

Each `.dip` file defines one `dataset_sources[...]` entry with a local root,
product regex, tags, and a link to a [dataset format](../formats/README.md).
The schema is [`schemas/dataset_source.dip`](../../schemas/dataset_source.dip).

These entries are example sources, not an exhaustive registry of products.
A format describes a file layout; a source describes one collection of files.
Several sources can link to the same format. For example, separate GOME-1
previous-release, development, and new-release directories can each have their
own source entry and `root`, all linking to `dataset_formats[gome_l1b]`.

The Dataset Browser indexes products from these roots. Register a new source
in the [runtime DIPfile](../../DIPfile); users can change an existing root in
`settings/runtime/overrides.dip` without changing project defaults.

The NASA Ocean Color, PACE OCI, VIIRS Dark Target, Sentinel-5P Level-2, GridMET, and Ocean Color Level-3 example
sources start with `root = "./"`. Set each source's root to your local product
directory in `overrides.dip` before indexing it. The Ocean Color and VIIRS
filename patterns capture a year but use day-of-year naming, so their browser
index does not infer a calendar month and day from the filename.
