/// Why a value does not fit its settings field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueFault {
    WrongType,
    OutOfRange,
}
