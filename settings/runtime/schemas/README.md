# Runtime schemas

These DIPL files define runtime types, defaults, descriptions, and validation.
`viewer.dip` is the root application schema; the other schemas cover themes,
dataset formats and sources, PICS sites, [plugins](plugins/README.md), and
[views](views/README.md).

The parent [runtime DIPfile](../DIPfile) registers them before loading values.
Keep field documentation here so the application and generated settings report
share one source of truth.
