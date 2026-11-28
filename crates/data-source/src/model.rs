//! Public data models shared by the file access and export APIs.

#[derive(Debug, Clone)]
pub enum Kind {
    Group,
    Dataset { shape: Vec<usize>, numeric: bool },
    Other,
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: Kind,
    pub children: Vec<Entry>,
}
#[derive(Debug)]
pub struct Values {
    pub data: Vec<f64>,
    pub fill_value: Option<f64>,
}
#[derive(Debug)]
pub struct Region {
    pub data: Vec<f64>,
    pub shape: Vec<usize>,
    pub fill_value: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct NumericEdit {
    pub coordinates: Vec<usize>,
    pub original: String,
    pub replacement: String,
}
#[derive(Debug)]
pub struct NumericApplyOutcome {
    pub applied: usize,
    pub error: Option<String>,
}
#[derive(Debug, Clone)]
pub struct DatasetInfo {
    pub shape: Vec<usize>,
    pub elements: u128,
    pub byte_size: u128,
    pub datatype: String,
    pub numeric: bool,
}
#[derive(Debug, Clone)]
pub struct AttributeValue {
    pub name: String,
    pub value: String,
    pub float_values: Option<Vec<f64>>,
}
#[derive(Debug, Clone)]
pub struct CoordinateAxis {
    pub name: String,
    pub length: usize,
    pub variable: Option<String>,
    pub units: Option<String>,
    pub first: Option<f64>,
    pub last: Option<f64>,
}
