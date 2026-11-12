use super::SpecialValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ValueVisibility {
    pub(crate) positive: bool,
    pub(crate) zero: bool,
    pub(crate) negative: bool,
    pub(crate) nan: bool,
    pub(crate) positive_inf: bool,
    pub(crate) negative_inf: bool,
    pub(crate) fill: bool,
}

impl Default for ValueVisibility {
    fn default() -> Self {
        Self {
            positive: true,
            zero: true,
            negative: true,
            nan: true,
            positive_inf: true,
            negative_inf: true,
            fill: true,
        }
    }
}

impl ValueVisibility {
    pub(crate) fn all_enabled(&self) -> bool {
        self.positive
            && self.zero
            && self.negative
            && self.nan
            && self.positive_inf
            && self.negative_inf
            && self.fill
    }

    pub(crate) fn set_all(&mut self, enabled: bool) {
        self.positive = enabled;
        self.zero = enabled;
        self.negative = enabled;
        self.nan = enabled;
        self.positive_inf = enabled;
        self.negative_inf = enabled;
        self.fill = enabled;
    }

    pub(crate) fn allows(&self, value: f64, fill_value: Option<f64>) -> bool {
        self.allows_class(value, SpecialValue::from_value(value, fill_value))
    }

    pub(crate) fn allows_class(&self, value: f64, special: Option<SpecialValue>) -> bool {
        match special {
            Some(SpecialValue::Fill) => self.fill,
            Some(SpecialValue::Nan) => self.nan,
            Some(SpecialValue::PositiveInf) => self.positive_inf,
            Some(SpecialValue::NegativeInf) => self.negative_inf,
            None if value > 0.0 => self.positive,
            None if value < 0.0 => self.negative,
            None => self.zero,
        }
    }
}
