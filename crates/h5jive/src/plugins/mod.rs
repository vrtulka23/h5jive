//! Built-in plugins live here, away from the inspector's view and loading state.
//! Add a plugin to `PLUGINS`, give it a DIPL schema and settings file, and
//! render it through `PluginContext`. A plugin can return a core Selection to
//! open any of the existing inspector views for that dataset.
mod path_comparison;
mod value_comparison;

use super::*;

pub(super) struct PluginContext<'a> {
    pub files: &'a [File],
    pub file_paths: &'a [PathBuf],
    pub settings: &'a Settings,
    pub selection: Option<&'a Selection>,
}

struct PluginDefinition {
    key: &'static str,
    title: &'static str,
    description: &'static str,
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

const PLUGINS: &[PluginDefinition] = &[
    PluginDefinition {
        key: "path_comparison",
        title: "A/B path comparison",
        description: "Compare paths, shapes, datatypes, and attributes in files A and B.",
        enabled: |settings| settings.path_comparison_enabled,
        icon: path_comparison::draw_icon,
        draw: draw_path_comparison,
    },
    PluginDefinition {
        key: "value_comparison",
        title: "Dataset value comparison",
        description: "Compare matching numeric arrays from A or B as a 2D difference or ratio.",
        enabled: |settings| settings.value_comparison_enabled,
        icon: value_comparison::draw_icon,
        draw: value_comparison::draw,
    },
];

#[derive(Default)]
pub(super) struct PluginLab {
    selected: usize,
    comparison: path_comparison::State,
    value_comparison: value_comparison::State,
}

impl PluginLab {
    pub fn invalidate(&mut self) {
        self.comparison.invalidate();
        self.value_comparison.invalidate();
    }

    pub fn is_busy(&mut self) -> bool {
        self.comparison.poll();
        self.value_comparison.poll();
        self.comparison.is_busy()
            || self.value_comparison.is_busy()
            || self.comparison.take_redraw()
            || self.value_comparison.take_redraw()
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        plot_ui: &PlotUi<'_>,
        files: &[File],
        file_paths: &[PathBuf],
        settings: &Settings,
        selection: Option<&Selection>,
    ) -> Option<Selection> {
        let available: Vec<_> = PLUGINS
            .iter()
            .enumerate()
            .filter(|(_, plugin)| (plugin.enabled)(settings))
            .collect();
        if available.is_empty() {
            ui.text_wrapped("No plugins are enabled. Enable one in settings/plugins.dip.");
            return None;
        }
        self.selected = self.selected.min(available.len() - 1);
        let palette = &settings.palette;
        {
            let _rail_background = ui.push_style_color(StyleColor::ChildBg, palette.panel);
            ui.child_window("plugin_icon_tabs")
                .size([76.0, 0.0])
                .build(ui, || {
                    for (position, (_, plugin)) in available.iter().enumerate() {
                        let clicked = ui.invisible_button(
                            format!("##plugin_{}", plugin.key),
                            [ui.content_region_avail()[0], 56.0],
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
                        ui.set_item_tooltip(format!("{} - {}", plugin.title, plugin.description));
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
                ui.text_wrapped(plugin.description);
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
                    },
                );
            });
        result
    }
}
