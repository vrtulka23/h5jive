# Troubleshooting

## Build cannot find HDF5

Install the HDF5 development library and make it visible to `pkg-config`. If
SNT3's build cannot find a nonstandard HDF5 installation, set
`SNT3_HDF5_LIB_DIR` and, if needed, `SNT3_HDF5_LIB_NAME`.

## Native SNT3 mismatch

The workspace pins `scinumtools3` 0.10.1, whose `scinumtools3-sys` crate builds
the matching bundled native source by default. Check that `SNT3_PREFIX` or
`SNT3_BUILD_DIR` is not forcing an older installation.

## World map has no pixels

Select a numeric dataset with discoverable latitude and longitude coordinates.
If you choose **Pixel edges** or **Filled pixels**, the dataset also needs both
coordinate bounds arrays. The bundled GOME-2 Band 3 sample is at
`MODE_NADIR/BAND_3/OBSERVATIONS/radiance`.

## Generate API documentation

```sh
cargo doc --workspace --no-deps --open
```

The app's internal modules are not all public API; use this guide and the source
tree for implementation details.
