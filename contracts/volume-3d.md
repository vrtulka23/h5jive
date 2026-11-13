# 3D volume contract

- The axes, volume, range controls, and tooltips use the same selected slice,
  pinned index, camera, and coordinate system. Dragging to rotate updates the
  volume as well as the axes. Panning moves them together.
- The volume and axes use the same perspective camera: a face closer to the
  viewer appears larger than an equal-sized distant face. Projected axis marks,
  voxel positions, and visible-region crop bounds stay aligned under rotation,
  pan, and zoom.
- The first volume preview starts loading immediately. A large volume uses a
  bounded sampled preview; a selection within the exact loader's limits uses
  its native voxels.
- Zoomed refinement reads only the requested visible region. New refinement
  starts after 150 ms without zoom input; another zoom cancels obsolete work.
  Rotation and pan are not held by the zoom delay.
- While a new ray trace loads, keep a compatible prior image visible. Zooming
  out uses a wider, lower-resolution image outside the refined crop when one is
  available. After the new ray trace completes, the entire visible part of the
  selected volume has coverage.
- Finer voxels replace coarse voxels only inside the finer data's bounds.
  Coarse voxels remain visible elsewhere; a late coarse result must not cover
  an available finer result.
- Ordinary and special values occupy their native voxel positions and are
  composited through the volume with the selected opacity. Near voxels retain
  full brightness and distant voxels dim according to the depth setting.

## Regression sequence

Use a small volume with known front, back, and special-value voxels. Rotate,
zoom in until a crop refines, zoom out before it completes, and pan. Check the
image while loading and after it settles: axes and volume move together, no
newly visible region is cut away after rendering, and fine data stays visible
over coarse data only within its bounds. Compare equal-sized near and far faces
and confirm the near face projects larger without separating from its axis box.
