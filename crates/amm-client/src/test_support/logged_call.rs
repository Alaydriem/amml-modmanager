/// One call the fake table's `log` received.
#[derive(Debug, Clone, PartialEq)]
pub struct LoggedCall {
    pub level: u8,
    pub target: String,
    pub message: String,
    pub fields_json: String,
    pub file: String,
    pub line: u32,
}
