# 1D profiles contract

- A profile point represents the selected dataset value at the displayed axis
  index and current pinned indexes. Multiple requested lines remain distinct.
- Single profile is the initial mode for multidimensional datasets: one axis
  varies and every other axis uses its pinned index. The comparison mode plots
  separate lines at selected indexes of one other axis.
- When the first dimension is identified as time and another dimension exists,
  the initial profile axis is the second dimension. Time stays pinned at its
  configured default index and can be changed with its pin control.
- Axis names, index ticks, tooltips, and the plotted values refer to the same
  dimension and source indexes. The horizontal axis is the chosen dimension's
  index; the vertical axis is the selected dataset's value.
- Zooming and resetting change plot limits without changing the source values.
  Changing the selected profile reads its values before drawing the new plot.
- Special-value visibility and colors follow the [shared contract](shared.md).
  A hidden class does not reappear because another line or zoom level loads.
- A new dataset or pin selection invalidates incompatible profiles. The view
  does not start 2D tiles or 3D volume reads.
- Invalid comparison indexes appear in a persistent overlay, separate from the
  controls only while the profile view is active. Correcting the input or
  removing the comparison axis clears it.

## Regression sequence

Plot two lines from a fixture, change one pinned index, zoom, and reset. Compare
sample points and tooltips with direct reads before and after each action.
