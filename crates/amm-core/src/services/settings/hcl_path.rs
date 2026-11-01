/// Dotted HCL paths such as `mini_map.zoom`.
pub struct HclPath;

impl HclPath {
    /// Splits a dotted path into identifiers. Returns `None` for an empty path, an empty segment or a segment
    /// that is not an HCL identifier.
    pub fn split(path: &str) -> Option<impl Iterator<Item = &str>> {
        if path.split('.').all(Self::is_identifier) { Some(path.split('.')) } else { None }
    }

    fn is_identifier(segment: &str) -> bool {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }
}
