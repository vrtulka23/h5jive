//! Reusable indexed-product range selection for export and preview.
use super::Product;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductRange {
    pub from_date: String,
    pub through_date: String,
    pub from_orbit: String,
    pub through_orbit: String,
}

impl ProductRange {
    pub fn matches(&self, product: &Product) -> bool {
        let date = format!("{}{}{}", product.year, product.month, product.day);
        let from = super::date_query(&self.from_date).unwrap_or_default();
        let through = super::date_query(&self.through_date).unwrap_or_default();
        let earliest = format!("{date:0<8}");
        let latest = format!("{date:9<8}");
        let orbit_at_least =
            |bound: &str| match (product.orbit.parse::<u64>(), bound.parse::<u64>()) {
                (Ok(value), Ok(bound)) => value >= bound,
                _ => product.orbit.as_str() >= bound,
            };
        let orbit_at_most = |bound: &str| match (product.orbit.parse::<u64>(), bound.parse::<u64>())
        {
            (Ok(value), Ok(bound)) => value <= bound,
            _ => product.orbit.as_str() <= bound,
        };
        (from.is_empty() || !date.is_empty() && latest >= from)
            && (through.is_empty() || !date.is_empty() && earliest <= through)
            && (self.from_orbit.is_empty()
                || !product.orbit.is_empty() && orbit_at_least(&self.from_orbit))
            && (self.through_orbit.is_empty()
                || !product.orbit.is_empty() && orbit_at_most(&self.through_orbit))
    }

    pub fn select<'a>(&self, products: &'a [Product]) -> Vec<&'a Product> {
        let mut selected: Vec<_> = products
            .iter()
            .filter(|product| self.matches(product))
            .collect();
        selected.sort_by(|a, b| {
            (&a.year, &a.month, &a.day)
                .cmp(&(&b.year, &b.month, &b.day))
                .then_with(|| match (a.orbit.parse::<u64>(), b.orbit.parse::<u64>()) {
                    (Ok(a), Ok(b)) => a.cmp(&b),
                    _ => a.orbit.cmp(&b.orbit),
                })
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_date_and_orbit_ranges() {
        let product = Product {
            filename: "one.nc".into(),
            relative_path: "one.nc".into(),
            year: "2024".into(),
            month: "01".into(),
            day: "05".into(),
            orbit: "0042".into(),
            size: 1,
        };
        assert!(ProductRange {
            from_date: "2024-01-01".into(),
            through_date: "2024-01-07".into(),
            from_orbit: "0040".into(),
            through_orbit: "0050".into()
        }
        .matches(&product));
        assert!(!ProductRange {
            from_date: "2024-01-06".into(),
            ..Default::default()
        }
        .matches(&product));
        assert!(ProductRange {
            from_date: "2024-01-01".into(),
            through_date: "2024-01-31".into(),
            from_orbit: "40".into(),
            through_orbit: "50".into()
        }
        .matches(&product));
        let yearly = Product {
            month: String::new(),
            day: String::new(),
            ..product
        };
        assert!(ProductRange {
            from_date: "2024-07-01".into(),
            through_date: "2024-07-07".into(),
            ..Default::default()
        }
        .matches(&yearly));
    }
}
