# H5Jive settings

This tree contains the DIPL configuration for H5Jive. It has two independent
projects: [build](build/README.md) supplies constants compiled into Rust, and
[runtime](runtime/README.md) supplies editable application settings. Each
project has a `DIPfile` that registers its schemas and input files.
The build project's resource limits cap the matching runtime budgets; an
override above a compiled ceiling prevents startup and identifies the setting.
Release packages include the runtime project, while the build project stays
in the source tree.

Start at a project's README to follow the configuration from schema to value
to Rust consumer. DIPL schemas define types, defaults, and constraints; source
files provide concrete values. The runtime project also links dataset formats
to browser sources. See [Settings and formats](../docs/src/settings-and-formats.md)
for the user-facing workflow.

With `snt dip format` on `PATH`, compilation checks and formats the `.dip`
files and `DIPfile`s in this tree. Validate the projects with
`snt dip validate --project settings/build/DIPfile` and
`snt dip validate --project settings/runtime/DIPfile`.

The combined [settings report](settings-report.pdf) contains the runtime and
build parameter references followed by the DIPL cheatsheet. Compilation
refreshes it when the settings change and `snt`, `pdflatex`, and `pdfjam` are
available. Run `./generate-settings-report.sh` from the repository root to
refresh it manually. The report is generated from project defaults, excluding
local `runtime/overrides.dip` values.
