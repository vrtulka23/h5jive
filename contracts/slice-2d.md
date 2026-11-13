# 2D slice contract

- The horizontal axis increases to the right and the vertical axis increases
  upward; their origin is at the lower left. Native row and column indexes,
  ticks, tooltips, and colored cells agree.
- The chosen axes and pinned indexes determine the plane. Changing them must
  not reuse incompatible tiles or an old value range.
- A large plane shows a sampled overview first. The overview uses source cells
  at its sample positions; it does not invent values by averaging. Zoomed tiles
  load only the visible region and show native cells where available.
- During zoom or pan, the highest resolution already available for each region
  stays visible. A coarser result never paints over a finer available region.
  Zooming out shows a valid overview outside a previously refined crop.
- New visible-region refinement starts after 150 ms without navigation input.
  A further zoom or pan cancels obsolete tile work. Initial overview loading
  starts immediately.
- Automatic colors exclude fill and nonfinite values and remain consistent
  between overview and refined tiles. Special-value classes retain their own
  colors and visibility settings.

## Regression sequence

Open a plane larger than the tile limit. Zoom in twice, zoom out once before
the second tile completes, then wait for refinement. Check coverage at every
step, verify that fine cells are not replaced by coarse ones, and compare a
tooltip and cell color with the corresponding source value.
