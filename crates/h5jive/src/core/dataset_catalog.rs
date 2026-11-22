//! A reusable index of numeric datasets in the loaded file slots.
use h5jive_data::{Entry, File, Kind};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DatasetChoice {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) shape: Vec<usize>,
}

impl DatasetChoice {
    pub(crate) fn label(&self) -> String {
        format!(
            "{} · {}  {:?}",
            if self.file == 0 { "A" } else { "B" },
            self.path,
            self.shape
        )
    }
}

pub(crate) fn numeric_datasets(files: &[File], minimum_rank: usize) -> Vec<DatasetChoice> {
    fn visit(slot: usize, entry: &Entry, minimum_rank: usize, out: &mut Vec<DatasetChoice>) {
        if let Kind::Dataset {
            shape,
            numeric: true,
        } = &entry.kind
        {
            if shape.len() >= minimum_rank && shape.iter().all(|&length| length > 0) {
                out.push(DatasetChoice {
                    file: slot,
                    path: entry.path.clone(),
                    shape: shape.clone(),
                });
            }
        }
        for child in &entry.children {
            visit(slot, child, minimum_rank, out);
        }
    }
    let mut out = Vec::new();
    for (slot, file) in files.iter().enumerate() {
        visit(slot, &file.tree, minimum_rank, &mut out);
    }
    out.sort_by(|a, b| (a.file, &a.path).cmp(&(b.file, &b.path)));
    out
}

pub(crate) fn compatible_datasets<'a>(
    choices: &'a [DatasetChoice],
    reference: &DatasetChoice,
) -> Vec<&'a DatasetChoice> {
    let mut out: Vec<_> = choices
        .iter()
        .filter(|choice| choice.shape == reference.shape)
        .collect();
    out.sort_by_key(|choice| {
        (
            choice.path != reference.path,
            choice.file == reference.file,
            choice.file,
            &choice.path,
        )
    });
    out
}
