# Runtime settings

This DIPL project configures the running application. [DIPfile](DIPfile)
registers [schemas](schemas/README.md), named view and plugin sources, dataset
formats, dataset sources, themes, and calibration sites. [viewer.dip](viewer.dip)
assembles the application tree from the named sources. The app loads the
evaluated project in `crates/h5jive/src/app/settings.rs`.

- [View values](viewers/README.md) and [plugin values](plugins/README.md)
  supply defaults used by `viewer.dip`.
- [Dataset metadata](dataset/README.md) connects product recognition to the
  Dataset Browser and geospatial export.
- [Themes](themes/README.md) and [PICS sites](pics/README.md) provide catalogs.

`overrides.dip` is a local, Git-ignored user override applied after project
defaults and preserved by the installer. `.cache/` contains generated browser
indexes; it is not DIPL source and is omitted from source documentation.
