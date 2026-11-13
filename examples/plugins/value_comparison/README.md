# Dataset value comparison demo

From the repository root:

```sh
cargo run -p h5jive --example generate_value_comparison_demo
cargo run -p h5jive -- examples/plugins/value_comparison/value_demo_a.hdf5 examples/plugins/value_comparison/value_demo_b.hdf5
```

In Plugin Lab, choose the **Dataset value comparison** icon. Select
`A · /field/plane` on the left and `B · /field/plane` on the right. The
difference is `−2` at every cell; the ratio is `A / B`, so the first cell is
`1 / 3`. The 3D `/field/cube` pair demonstrates pinning the remaining axis.
`/field/reference` provides compatible A/A and B/B choices.
`/field/other_shape` has different shapes between files and therefore is not
offered as a cross-file pair. Comparing A's plane with B's
`/field/zero_reference` in ratio mode excludes cells whose denominator is zero.

The 2D preview samples matching source positions when a plane exceeds the
configured cell limit. Fill values and non-finite inputs are excluded.
