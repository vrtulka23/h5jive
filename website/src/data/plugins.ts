export const pluginPages = [
  {
    id: 'path-comparison',
    name: 'A/B path comparison',
    summary: 'Line up the group and dataset paths in two loaded files. See where structure and metadata agree, differ, or exist on only one side.',
    details: ['Paths and object kinds', 'Dataset shapes and datatypes', 'Group and dataset attributes'],
    setting: 'path_comparison_enabled',
    source: 'https://github.com/vrtulka23/h5jive/blob/main/crates/h5jive/src/plugins/path_comparison.rs',
    demo: 'https://github.com/vrtulka23/h5jive/tree/main/examples/plugins/path_comparison',
    mark: ['A', '⇄', 'B'],
  },
  {
    id: 'value-comparison',
    name: 'Dataset value comparison',
    summary: 'Choose two compatible numeric datasets from A or B and explore their difference or ratio as a 2D slice.',
    details: ['A/B, A/A, or B/B pairs', 'Matching shapes only', 'Two displayed dimensions and pinned indexes'],
    setting: 'value_comparison_enabled',
    source: 'https://github.com/vrtulka23/h5jive/blob/main/crates/h5jive/src/plugins/value_comparison.rs',
    demo: 'https://github.com/vrtulka23/h5jive/tree/main/examples/plugins/value_comparison',
    mark: ['A', '÷', 'B'],
  },
];
