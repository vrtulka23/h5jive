# View contracts

These contracts describe the behavior a user may rely on in each H5Jive view.
They are acceptance criteria for changes to loading, navigation, rendering, or
dataset selection. A contract describes the expected result; its presence here
does not mean an automated test already covers it.

## How to use them

1. Identify the affected view and the [shared contract](shared.md).
2. Add a regression test for the changed behavior. Prefer a small deterministic
   fixture and a sequence of user actions over a test of one helper function.
3. Verify the visible result, the loading state, and the result after loading
   completes. Use a large local product for performance checks when relevant.
4. Update the contract when the intended behavior changes.

The bundled `examples/arrays/space_demo.hdf5` and
`examples/satellite/gome2_band3_20130106_gome.nc` provide stable fixtures. Local full
S5P and GOME-2 products are useful for manual scale checks but must not be
required by the ordinary test suite.

## Views

- [Shared behavior](shared.md)
- [Data table](data.md)
- [1D profiles](profiles-1d.md)
- [2D slice](slice-2d.md)
- [3D volume](volume-3d.md)
- [World map and globe](geospatial.md)
