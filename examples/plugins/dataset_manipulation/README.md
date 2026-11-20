# Dataset manipulation demo

Generate two small HDF5 files from the repository root:

```sh
cargo run -p h5jive --example generate_dataset_manipulation_demo
cargo run -p h5jive -- examples/plugins/dataset_manipulation/manipulation_a.hdf5 examples/plugins/dataset_manipulation/manipulation_b.hdf5
```

In Plugin Lab, select **Dataset manipulation**. Try a slice of
`/measurements/counts` with axis 0 start `1`, stop `3`, step `1` and axis 1
start `0`, stop `4`, step `2`; the result is a 2 × 2 integer dataset with
values `[4, 6, 8, 10]`. Subtract B from A to obtain a 3 × 4 integer dataset
of `-2`; divide A by B to obtain floating point values. Save to a new dataset
path in A or B, or enter a new `.hdf5` filename.
