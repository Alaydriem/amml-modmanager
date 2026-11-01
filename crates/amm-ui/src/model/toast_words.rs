use amml_i18n::text::Text;

/// The words of one toast: a title, the sentence under it, and a hint line.
#[derive(Debug, Clone)]
pub struct ToastWords {
    pub title: Text,
    pub message: Option<Text>,
    pub hint: Option<Text>,
}
