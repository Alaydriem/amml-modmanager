/// Why a part of the overlay could not start: a pipeline object of `LayerRenderer::new`, the DXGI probe, a hook,
/// or a second install.
#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    #[error("could not create the {object}: {source}")]
    Create {
        object: &'static str,
        #[source]
        source: windows::core::Error,
    },
    #[error("the swap chain probe failed at {step}: {source}")]
    Probe {
        step: &'static str,
        #[source]
        source: windows::core::Error,
    },
    #[error("the hook installer refused: {reason}")]
    Hook { reason: String },
    #[error("an overlay is already installed in this process")]
    AlreadyInstalled,
    #[error("an earlier overlay was left alive by a shutdown that timed out")]
    Leaked,
}

impl OverlayError {
    /// The object a successful `Create*` call wrote to `out`, or the error of the call. A call that succeeds and
    /// writes nothing is `E_POINTER`.
    pub(crate) fn created<T>(
        object: &'static str,
        result: windows::core::Result<()>,
        out: Option<T>,
    ) -> Result<T, OverlayError> {
        match (result, out) {
            (Ok(()), Some(created)) => Ok(created),
            (Ok(()), None) => {
                Err(OverlayError::Create { object, source: windows::Win32::Foundation::E_POINTER.into() })
            }
            (Err(source), _) => Err(OverlayError::Create { object, source }),
        }
    }
}
