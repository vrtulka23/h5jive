# Dataset value comparison demo

From the repository root:

```sh
cargo run -p h5jive --example generate_value_comparison_demo
cargo run -p h5jive -- examples/plugins/value_comparison/value_demo_a.hdf5 examples/plugins/value_comparison/value_demo_b.hdf5
```

In Plugin Lab, choose the **Dataset value comparison** icon. Select
`A · /field/plane` on the left and `B · /field/plane` on the right. Both planes
contain a broad Gaussian peak and a smaller peak. Their positions and strengths
change in B, so the difference heatmap shows paired positive and negative lobes
instead of a flat color. The ratio view shows the same changes relative to B.
The main pair also has separate rectangular patches: A contains **NaN** and
`_FillValue` (`−9999`), while B contains **+Inf**, **−Inf**, and zeros. The
special values panel reports 120 cells in each of the four classes in
difference mode. In ratio mode, B's zero patch adds 120 more NaN results.
Toggle the class buttons to check their colored patches on the heatmap.
The shared finite patch in A and B gives a zero difference. Use the **+**,
**0**, and **-** buttons above the special value controls to filter ordinary
results by sign; each button shows its cell count. **None** hides every value
class, and **All** restores them.
The 3D `/field/cube` pair shows a drifting peak across three layers and
demonstrates pinning the remaining axis.
`/field/reference` provides compatible A/A and B/B choices.
`/field/other_shape` has different shapes between files and therefore is not
offered as a cross-file pair. Comparing A's plane with B's
`/field/zero_reference` in ratio mode excludes cells whose denominator is zero.

The 2D preview samples matching source positions when a plane exceeds the
configured cell limit. Fill values and non-finite inputs are excluded.
