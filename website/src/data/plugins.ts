export const pluginPages = [
  {
    id: 'path-comparison',
    name: 'A/B path comparison',
    summary: 'Line up the group and dataset paths in two loaded files. See where structure and metadata agree, differ, or exist on only one side.',
    details: ['Paths and object kinds', 'Dataset shapes and datatypes', 'Group and dataset attributes'],
    setting: 'path_comparison_enabled',
    source: 'https://github.com/vrtulka23/h5jive/blob/main/crates/h5jive/src/plugins/path_comparison.rs',
  },
];
