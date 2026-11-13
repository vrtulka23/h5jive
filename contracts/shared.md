# Shared view contract

## Selection and availability

- Selecting a numeric dataset updates every view's source path, shape, axes,
  attributes, and value classes. Results from the previous selection must not
  appear as results for the new dataset.
- The selected item in the file tree remains visibly distinct from unselected
  and hovered items in every theme. Its label, file or dataset icon, and geo
  marker remain readable.
- A view loads only work needed for the active view. Switching away cancels or
  ignores its pending work, and switching back may reuse compatible cached data.
- World map and globe are available only when geolocation is supported. If a
  new dataset lacks geolocation while either is active, the app switches to
  Data view.
- A command-line `--view-*` option selects the requested view for an explicit
  `FILE:/DATASET` selection; unsupported geospatial selections fall back to
  Data view.

## Loading and continuity

- Loading may refine an image progressively. A new request must not erase a
  valid image of the same dataset while its replacement is pending.
- A completed request for an old selection or navigation state must not replace
  the current result. A failed request reports an error rather than an endless
  loading indicator.
- The loading indicator names the active work, including refinement or loss of
  resolution where applicable. It disappears when that work finishes or is
  canceled.
- Initial loading starts without a navigation delay. Navigation delays apply
  only to the work identified in the individual view contracts.

## Values and controls

- Ordinary, fill, NaN, positive infinity, and negative infinity remain distinct
  classes in plots. Visibility controls affect only the selected classes;
  **All/None** changes all classes together.
- Changing a display setting must not silently change source values or the
  selected dataset. A displayed value and its tooltip must refer to the same
  source cell.

## Regression sequence

Open a bundled dataset, switch between views, change the selected dataset
while a load is pending, and return. Check that the visible source, controls,
loading text, and final result all match the current selection.
