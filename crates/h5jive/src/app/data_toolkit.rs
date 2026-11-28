use super::*;

impl SpecialValue {
    pub(super) fn color(self, palette: &Palette) -> [f32; 4] {
        match self {
            Self::Fill => palette.fill,
            Self::Nan => palette.nan,
            Self::PositiveInf => palette.positive_inf,
            Self::NegativeInf => palette.negative_inf,
        }
    }
}

impl App {
    pub(super) fn ensure_attributes_loaded(&mut self, selection: &Selection) {
        if self.dataset.attributes_cache.is_some() {
            return;
        }
        let result = self.session.files[selection.file]
            .attributes(&selection.path)
            .map_err(|err| err.to_string());
        if let Err(error) = &result {
            self.show_notice(
                NoticeKind::Error,
                format!("Could not read dataset attributes: {error}"),
            );
        }
        self.dataset.attributes_cache = Some(result);
    }

    pub(super) fn ensure_coordinates_loaded(&mut self, selection: &Selection) {
        if self.dataset.coordinates_cache.is_some() {
            return;
        }
        let result = self.session.files[selection.file]
            .coordinate_axes(&selection.path)
            .map_err(|err| err.to_string());
        if let Err(error) = &result {
            self.show_notice(
                NoticeKind::Error,
                format!("Could not read coordinate metadata: {error}"),
            );
        }
        self.dataset.coordinates_cache = Some(result);
    }

    pub(super) fn cached_coordinate_label(&self, axis: usize, index: usize) -> String {
        self.dataset
            .coordinate_value_cache
            .get(&(axis, index))
            .and_then(Option::as_ref)
            .map_or_else(|| index.to_string(), |value| format!("{index} ({value})"))
    }

    pub(super) fn cached_pinned_coordinate_labels(&self, pinned: &[usize]) -> Vec<String> {
        pinned
            .iter()
            .enumerate()
            .map(|(axis, &index)| {
                let label = self.cached_coordinate_label(axis, index);
                if label == index.to_string() {
                    "index".into()
                } else {
                    label
                }
            })
            .collect()
    }

    pub(super) fn pinned_coordinate_labels(&mut self, pinned: &[usize]) -> Vec<String> {
        let selection = self.session.selection.clone();
        pinned
            .iter()
            .enumerate()
            .map(|(axis, &index)| {
                let label = selection.as_ref().map_or_else(
                    || index.to_string(),
                    |selection| self.coordinate_label(selection, axis, index),
                );
                if label == index.to_string() {
                    "index".into()
                } else {
                    label
                }
            })
            .collect()
    }
    pub(super) fn coordinate_axes_for(
        &mut self,
        selection: &Selection,
    ) -> Option<&[CoordinateAxis]> {
        self.ensure_coordinates_loaded(selection);
        self.dataset
            .coordinates_cache
            .as_ref()?
            .as_ref()
            .ok()
            .map(Vec::as_slice)
    }
    pub(super) fn coordinate_labels(
        &mut self,
        selection: &Selection,
        axis: usize,
        start: usize,
        count: usize,
    ) -> Vec<String> {
        if count == 0 {
            return Vec::new();
        }
        let variable = self
            .coordinate_axes_for(selection)
            .and_then(|axes| axes.get(axis).and_then(|axis| axis.variable.clone()));
        let Some(variable) = variable else {
            return (start..start + count)
                .map(|index| index.to_string())
                .collect();
        };
        if (start..start + count).any(|index| {
            !self
                .dataset
                .coordinate_value_cache
                .contains_key(&(axis, index))
        }) {
            if self.dataset.coordinate_value_cache.len() + count > 8192 {
                self.dataset.coordinate_value_cache.clear();
            }
            let rank = match self.session.files[selection.file].dataset_info(&variable) {
                Ok(info) => info.shape.len(),
                Err(error) => {
                    self.show_notice(
                        NoticeKind::Error,
                        format!("Could not inspect coordinate variable {variable}: {error}"),
                    );
                    1
                }
            };
            let values = if rank == 2 {
                self.session.files[selection.file].region(&variable, &[0, start], &[1, count])
            } else {
                self.session.files[selection.file].region(&variable, &[start], &[count])
            };
            if let Err(error) = &values {
                self.show_notice(
                    NoticeKind::Error,
                    format!("Could not read coordinate values from {variable}: {error}"),
                );
            }
            for offset in 0..count {
                let value = values.as_ref().ok().and_then(|region| {
                    region.data.get(offset).map(|&value| {
                        if region.fill_value == Some(value) {
                            "Fill".to_owned()
                        } else {
                            format_value(value, self.settings.display_decimals)
                        }
                    })
                });
                self.dataset
                    .coordinate_value_cache
                    .insert((axis, start + offset), value);
            }
        }
        (start..start + count)
            .map(|index| {
                self.dataset
                    .coordinate_value_cache
                    .get(&(axis, index))
                    .and_then(|value| value.as_ref())
                    .map_or_else(|| index.to_string(), |value| format!("{index} ({value})"))
            })
            .collect()
    }
    pub(super) fn coordinate_label(
        &mut self,
        selection: &Selection,
        axis: usize,
        index: usize,
    ) -> String {
        self.coordinate_labels(selection, axis, index, 1)
            .into_iter()
            .next()
            .unwrap_or_else(|| index.to_string())
    }
}
