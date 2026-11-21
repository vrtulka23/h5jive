//! Built-in plugins live here, away from the inspector's view and loading state.
//! Add a plugin to `PLUGINS`, give it a DIPL schema and settings file, and
//! render it through `PluginContext`. A plugin can return a core Selection to
//! open any of the existing inspector views for that dataset.
mod dataset_manipulation;
mod path_comparison;
mod spatiotemporal_subset;
mod value_comparison;

use super::*;

mod compiled_settings {
    include!(concat!(env!("OUT_DIR"), "/compiled_settings.rs"));
}

pub(crate) struct PluginLayout {
    pub(crate) rail_width: f32,
    pub(crate) button_height: f32,
}

pub(super) struct PluginContext<'a> {
    pub files: &'a mut [File],
    pub file_paths: &'a [PathBuf],
    pub settings: &'a Settings,
    pub selection: Option<&'a Selection>,
    pub write_allowed: [bool; 2],
    pub browser: &'a DatasetBrowser,
    pub picker_request: &'a mut Option<(FilePickerTarget, String)>,
}

struct PluginDefinition {
    key: &'static str,
    title: &'static str,
    description: &'static str,
    help: &'static str,
    enabled: fn(&Settings) -> bool,
    icon: fn(&DrawListMut<'_>, [f32; 2], [f32; 2], [f32; 4]),
    draw: fn(&mut PluginLab, &Ui, &PlotUi<'_>, PluginContext<'_>) -> Option<Selection>,
}

fn draw_path_comparison(
    lab: &mut PluginLab,
    ui: &Ui,
    _plot_ui: &PlotUi<'_>,
    context: PluginContext<'_>,
) -> Option<Selection> {
    path_comparison::draw(lab, ui, context)
}

fn draw_dataset_manipulation(
    lab: &mut PluginLab,
    ui: &Ui,
    _plot_ui: &PlotUi<'_>,
    context: PluginContext<'_>,
) -> Option<Selection> {
    dataset_manipulation::draw(lab, ui, context)
}

fn draw_spatiotemporal_subset(
    lab: &mut PluginLab,
    ui: &Ui,
    _plot_ui: &PlotUi<'_>,
    context: PluginContext<'_>,
) -> Option<Selection> {
    spatiotemporal_subset::draw(lab, ui, context)
}

const PLUGINS: &[PluginDefinition] = &[
    PluginDefinition {
        key: "path_comparison",
        title: "A/B path comparison",
        description: "Compare paths, shapes, datatypes, and attributes in files A and B.",
        help: "Compare the loaded files A and B by path, object kind, shape, datatype, and displayed attributes. Use the status buttons to filter paths; click a path to expand its details on both sides. Click a missing-path arrow to copy one object, or use Copy missing A -> B / B -> A for all missing objects. Click a Different icon to choose which side should replace the other at that path. Replacing a dataset also copies its values; replacing a group also replaces its children. Dataset values are not compared here.",
        enabled: |settings| settings.path_comparison_enabled,
        icon: path_comparison::draw_icon,
        draw: draw_path_comparison,
    },
    PluginDefinition {
        key: "value_comparison",
        title: "Dataset value comparison",
        description: "Compare matching numeric arrays from A or B as a 2D difference or ratio.",
        help: "Choose a numeric dataset on the left, then pick a same-shaped dataset on the right from either file. Compare their difference or ratio in a 2D view. Select two dimensions to display and pin the others. Invalid values, fill values, and division by zero are excluded from the calculation. The buttons beside the plot show or hide positive, zero, and negative results and special classes; their counts refer to cells in the sampled 2D preview.",
        enabled: |settings| settings.value_comparison_enabled,
        icon: value_comparison::draw_icon,
        draw: value_comparison::draw,
    },
    PluginDefinition {
        key: "dataset_manipulation",
        title: "Dataset manipulation",
        description: "Create a new dataset from a slice or arithmetic on two datasets.",
        help: "Choose a numeric dataset and slice it with independent start, stop, and step controls, or combine two matching datasets by addition, subtraction, multiplication, or division. Give the result a new dataset path in file A or B, or save it in a new HDF5 file. Existing datasets and files are protected. Slices keep their numeric datatype; integer addition, subtraction, and multiplication write i64 when the result fits, while division writes floating point values. Metadata attributes are not copied.",
        enabled: |settings| settings.dataset_manipulation_enabled,
        icon: dataset_manipulation::draw_icon,
        draw: draw_dataset_manipulation,
    },
    PluginDefinition {
        key: "spatiotemporal_subset",
        title: "Spatiotemporal subset",
        description: "Preview product coverage and export geographic and time subsets to NetCDF-4.",
        help: "Choose a Dataset Browser source, a date or orbit range, and fields from a representative NetCDF product. Preview the combined coverage on a world map or globe. Drag on the map or enter latitude and longitude bounds. Native export keeps each selected product in its own NetCDF group, with cropped spatial dimensions, coordinates, attributes, and an optional time subset. Mosaic export averages overlapping samples into one regular latitude/longitude grid and writes sample counts. Exports create a new .nc file and never replace an existing file.",
        enabled: |settings| settings.spatiotemporal_subset_enabled,
        icon: spatiotemporal_subset::draw_icon,
        draw: draw_spatiotemporal_subset,
    },
];

#[derive(Default)]
pub(super) struct PluginLab {
    selected: usize,
    comparison: path_comparison::State,
    value_comparison: value_comparison::State,
    dataset_manipulation: dataset_manipulation::State,
    spatiotemporal_subset: spatiotemporal_subset::State,
}

impl PluginLab {
    pub fn set_picker_path(&mut self, target: FilePickerTarget, path: String) {
        match target {
            FilePickerTarget::ManipulationOutput => self.dataset_manipulation.new_file = path,
            FilePickerTarget::SubsetOutput => self.spatiotemporal_subset.output = path,
            _ => {}
        }
    }
    pub fn help_topic(&self, settings: &Settings) -> (&'static str, &'static str) {
        let available: Vec<_> = PLUGINS
            .iter()
            .filter(|plugin| (plugin.enabled)(settings))
            .collect();
        available
            .get(self.selected.min(available.len().saturating_sub(1)))
            .map_or(
                (
                    "Plugin Lab",
                    "Enable a plugin in settings/runtime/plugins.dip to use it here.",
                ),
                |plugin| (plugin.title, plugin.help),
            )
    }
    pub fn has_sync_request(&self) -> bool {
        self.comparison.has_sync_request()
    }

    pub fn is_syncing(&self) -> bool {
        self.comparison.is_syncing()
    }

    pub fn start_requested_sync(&mut self, paths: &[PathBuf]) {
        self.comparison.start_requested_sync(paths);
    }
    pub fn take_reload(&mut self) -> Option<(usize, PathBuf)> {
        self.comparison.take_reload().or_else(|| {
            self.dataset_manipulation
                .take_reload()
                .then(|| (0, PathBuf::new()))
        })
    }
    pub fn invalidate(&mut self) {
        self.comparison.invalidate();
        self.value_comparison.invalidate();
        self.dataset_manipulation.invalidate();
        self.spatiotemporal_subset.invalidate();
    }

    pub fn is_busy(&mut self) -> bool {
        self.comparison.poll();
        self.value_comparison.poll();
        let manipulation_busy = self.dataset_manipulation.is_busy();
        let subset_busy = self.spatiotemporal_subset.is_busy();
        self.comparison.is_busy()
            || self.value_comparison.is_busy()
            || manipulation_busy
            || subset_busy
            || self.comparison.take_redraw()
            || self.value_comparison.take_redraw()
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        files: &mut [File],
        file_paths: &[PathBuf],
        settings: &Settings,
        selection: Option<&Selection>,
        write_allowed: [bool; 2],
        browser: &DatasetBrowser,
        picker_request: &mut Option<(FilePickerTarget, String)>,
    ) -> Option<Selection> {
        let available: Vec<_> = PLUGINS
            .iter()
            .enumerate()
            .filter(|(_, plugin)| (plugin.enabled)(settings))
            .collect();
        if available.is_empty() {
            ui.text_wrapped("No plugins are enabled. Enable one in settings/runtime/plugins.dip.");
            return None;
        }
        self.selected = self.selected.min(available.len() - 1);
        let palette = &settings.palette;
        {
            let _rail_background = ui.push_style_color(StyleColor::ChildBg, palette.panel);
            ui.child_window("plugin_icon_tabs")
                .size([compiled_settings::PLUGIN_LAYOUT.rail_width, 0.0])
                .build(ui, || {
                    for (position, (_, plugin)) in available.iter().enumerate() {
                        let clicked = ui.invisible_button(
                            format!("##plugin_{}", plugin.key),
                            [
                                ui.content_region_avail()[0],
                                compiled_settings::PLUGIN_LAYOUT.button_height,
                            ],
                        );
                        let hovered = ui.is_item_hovered();
                        if clicked {
                            self.selected = position;
                        }
                        let min = ui.item_rect_min();
                        let max = ui.item_rect_max();
                        let active = self.selected == position;
                        let background = if active {
                            palette.view_selected
                        } else if hovered {
                            palette.view_hover
                        } else {
                            palette.view_idle
                        };
                        let border = if active {
                            palette.view_border_selected
                        } else {
                            palette.view_border_idle
                        };
                        let draw = ui.get_window_draw_list();
                        draw_control_surface(
                            &draw,
                            min,
                            max,
                            background,
                            border,
                            ui.clone_style().frame_rounding(),
                        );
                        (plugin.icon)(&draw, min, max, palette.view_stroke);
                        item_tip(
                            ui,
                            settings.tutorial_tips,
                            &format!("{} - {}", plugin.title, plugin.description),
                            "Click to open this tool.",
                        );
                    }
                });
        }
        ui.same_line_with_spacing(0.0, PANEL_GAP);
        let plugin = available[self.selected].1;
        let mut result = None;
        ui.child_window("plugin_tab_content")
            .size([0.0, 0.0])
            .build(ui, || {
                ui.text(plugin.title);
                ui.separator();
                result = (plugin.draw)(
                    self,
                    ui,
                    plot_ui,
                    PluginContext {
                        files,
                        file_paths,
                        settings,
                        selection,
                        write_allowed,
                        browser,
                        picker_request,
                    },
                );
            });
        result
    }
}
