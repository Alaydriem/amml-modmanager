/// A panic payload whose drop panics again.
pub struct PanickingPayload;

impl Drop for PanickingPayload {
    fn drop(&mut self) {
        panic!("payload drop failed on purpose")
    }
}
