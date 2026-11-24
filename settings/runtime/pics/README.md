# Calibration and reference sites

These DIPL catalogs define the PICS regions and reference stations shown by
geospatial views. Their schemas are in the parent project's
[`schemas/`](../schemas/README.md) directory. The [runtime DIPfile](../DIPfile)
registers each catalog, and `app/settings.rs` reads the resulting site entries.

Site geometry, metadata, and source URLs belong with the individual catalog
entry so generated documentation can attribute them correctly.
