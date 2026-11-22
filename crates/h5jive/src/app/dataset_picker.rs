//! Searchable numeric dataset selection shared by plugins.
use super::*;

pub(super) fn dataset_choice_picker(
    ui: &Ui,
    id: &str,
    title: &str,
    choices: &[&DatasetChoice],
    selected: Option<&DatasetChoice>,
    search: &mut String,
    tutorial_tips: bool,
) -> Option<DatasetChoice> {
    ui.text(title);
    let preview = selected.map_or_else(|| "Select a dataset".to_owned(), DatasetChoice::label);
    ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
    let combo = ui.begin_combo(format!("##dataset_select_{id}"), &preview);
    item_tip(
        ui,
        tutorial_tips,
        &preview,
        "Choose a numeric dataset from file A or B.",
    );
    let mut picked = None;
    if let Some(_combo) = combo {
        ui.set_next_item_width(ui.content_region_avail()[0].max(1.0));
        ui.input_text(format!("Search##{id}"), search).build();
        let query = search.trim().to_lowercase();
        let visible = choices
            .iter()
            .filter(|choice| {
                query.is_empty()
                    || choice.path.to_lowercase().contains(&query)
                    || (if choice.file == 0 { "a" } else { "b" }).contains(&query)
            })
            .copied()
            .collect::<Vec<_>>();
        ui.text_disabled(format!("{} matching datasets", visible.len()));
        ui.child_window(format!("options##{id}"))
            .size([0.0, 220.0])
            .build(ui, || {
                if visible.is_empty() {
                    ui.text_disabled("No datasets match this search.");
                }
                let mut clipper = ListClipper::new(visible.len()).begin(ui);
                while clipper.step() {
                    for index in clipper.display_start()..clipper.display_end() {
                        let choice = visible[index];
                        if ui
                            .selectable_config(format!("{}##{id}_{index}", choice.label()))
                            .selected(selected == Some(choice))
                            .build()
                        {
                            picked = Some(choice.clone());
                        }
                        if ui.is_item_hovered() {
                            item_plain_tip(ui, &choice.label());
                        }
                    }
                }
            });
    }
    picked
}
