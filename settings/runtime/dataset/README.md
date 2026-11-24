# Dataset metadata

This part of the runtime project describes supported product layouts and local
repositories. [Formats](formats/README.md) define geolocation, optional time
coordinates, and scan flags. [Sources](sources/README.md) link a format to a
repository root and a product filename pattern.

The parent [runtime DIPfile](../DIPfile) registers both kinds of entry. The
Dataset Browser reads sources; map views and subsetting use the linked format.
