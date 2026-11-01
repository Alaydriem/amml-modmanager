/// What the schema check did to a config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    /// The value was outside its range and was set to the nearest bound.
    Clamped { path: String },
    /// The value was missing or not valid and was set to its default.
    Defaulted { path: String },
    /// The text is not HCL.
    Unparseable,
}
