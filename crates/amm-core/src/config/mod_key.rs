/// How a mod id is written as an HCL attribute name in the manager's config. An id `@<namespace>/<name>` becomes
/// `<namespace>_<name>`. A namespace or name never holds `_`, so the form is unambiguous.
pub struct ModKey;

impl ModKey {
    const SEPARATOR: char = '_';

    /// The attribute name of `id`: the leading `@` is dropped and the `/` becomes `_`.
    pub fn attribute(id: &str) -> String {
        id.strip_prefix('@').unwrap_or(id).replace('/', &Self::SEPARATOR.to_string())
    }

    /// The id an attribute name stands for, or `None` when it is not `<namespace>_<name>`.
    pub fn id(attribute: &str) -> Option<String> {
        let (namespace, name) = attribute.split_once(Self::SEPARATOR)?;
        if namespace.is_empty() || name.is_empty() || name.contains(Self::SEPARATOR) {
            return None;
        }
        Some(format!("@{namespace}/{name}"))
    }
}
