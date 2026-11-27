use amml_api::ApiError;

/// Why a part of the overlay could not run: a pipeline object of `LayerRenderer::new`, or the game picture copy of
/// `SurfaceRenderer::refresh_game_picture`.
#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    #[error("could not create the {object}: {source}")]
    Create {
        object: &'static str,
        #[source]
        source: windows::core::Error,
    },
    #[error("the overlay has no device objects for this surface")]
    NoDevice,
    #[error("a back buffer of {width} x {height} in format {format} cannot be copied into a game picture")]
    PictureSurface { width: u32, height: u32, format: u32 },
    #[error("the loader did not copy the back buffer: {source}")]
    Copy {
        #[source]
        source: ApiError,
    },
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
