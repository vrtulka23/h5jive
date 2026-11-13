# 1D profiles contract

- A profile point represents the selected dataset value at the displayed axis
  index and current pinned indexes. Multiple requested lines remain distinct.
- Axis names, index ticks, tooltips, and the plotted values refer to the same
  dimension and source indexes.
- Zooming and resetting change plot limits without changing the source values.
  The best available plot stays visible while a replacement loads.
- Special-value visibility and colors follow the [shared contract](shared.md).
  A hidden class does not reappear because another line or zoom level loads.
- A new dataset or pin selection invalidates incompatible profiles. The view
  does not start 2D tiles or 3D volume reads.

## Regression sequence

Plot two lines from a fixture, change one pinned index, zoom, and reset. Compare
sample points and tooltips with direct reads before and after each action.
