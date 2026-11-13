# Data table contract

- The table displays the selected dataset's native values at the shown row,
  column, and pinned indexes. Reordering axes or changing a pin changes the
  displayed cells accordingly.
- The initial axes follow the dataset dimensions. If the first dimension is
  time and at least two dimensions remain after it, the table shows the next
  two and pins time. A one-dimensional remainder appears as one column.
- Scrolling or paging reads only the cells needed for the visible table region.
  Changing the selection must not leave rows from the previous dataset.
- The Attributes tab is first and opens by default. Statistics and Distribution
  begin loading when their tabs are opened. Counts in Statistics use the total
  number of dataset elements as the percentage denominator.
- Coordinates, dimension names, units, fill values, and resolved dimension
  references shown in details agree with the selected dataset's metadata.

## Regression sequence

Open a known multidimensional fixture, record a displayed value and its index,
swap the displayed axes, change a pinned index, and compare each resulting cell
with a direct source read. Switch datasets while a region request is pending;
the table and details must both show the new selection.
