use super::ModIcon;

/// A mod's icon with the number that names this set. The number never repeats within one book.
#[derive(Debug, Clone, PartialEq)]
pub struct IconEntry {
    pub icon: ModIcon,
    pub generation: u64,
}
