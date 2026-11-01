use amm_core::structs::render::RenderSize;

use crate::services::{Boundary, HostCell};

/// Render thread, in `ResizeBuffers` of the game chain, and window thread, in `WM_SIZE`. Writes to `out_width` and
/// `out_height` the size to use in place of `width` x `height`: the forced render size while a `RenderSize` holder
/// forces one, else the asked size unchanged. Returns true while a size is forced. It reads the lock-free state and
/// never waits on the host lock. A null out pointer gives false, and nothing is written.
///
/// # Safety
///
/// `out_width` and `out_height` are null or writable for the call.
pub unsafe extern "C" fn amm_host_render_size(
    width: u32,
    height: u32,
    out_width: *mut u32,
    out_height: *mut u32,
) -> bool {
    let result = Boundary::guard(|| {
        let (out_width, out_height) = (Boundary::writable(out_width)?, Boundary::writable(out_height)?);
        let (forced, size) = HostCell::global().render_size_for(RenderSize::new(width, height));
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the caller
        // gives
        unsafe {
            out_width.write(size.width);
            out_height.write(size.height);
        }
        Ok(forced)
    });
    result.unwrap_or(false)
}

/// Window thread, in `WM_SIZE` before the game sees it. Writes the size the game gets in place of `width` x
/// `height`: the forced render size while one is in effect, so a resize of the window keeps it; the asked size for a
/// minimised window (`minimised`, `SIZE_MINIMIZED`) or with nothing forced. Returns true when the size is replaced.
/// Lock-free, as `amm_host_render_size`. A null out pointer gives false, and nothing is written.
///
/// # Safety
///
/// `out_width` and `out_height` are null or writable for the call.
pub unsafe extern "C" fn amm_host_window_size(
    minimised: bool,
    width: u32,
    height: u32,
    out_width: *mut u32,
    out_height: *mut u32,
) -> bool {
    let result = Boundary::guard(|| {
        let (out_width, out_height) = (Boundary::writable(out_width)?, Boundary::writable(out_height)?);
        let (forced, size) = HostCell::global().window_size_for(minimised, RenderSize::new(width, height));
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the caller
        // gives
        unsafe {
            out_width.write(size.width);
            out_height.write(size.height);
        }
        Ok(forced)
    });
    result.unwrap_or(false)
}

/// Window thread, on `WM_DPICHANGED`, `WM_DISPLAYCHANGE` and `WM_WINDOWPOSCHANGED` of the game window. While a
/// render size is forced, the next frame sends it to the game again as `WM_SIZE`; with nothing forced it does
/// nothing. Lock-free, as `amm_host_render_size`.
pub extern "C" fn amm_host_window_changed() {
    let result = Boundary::guard(|| {
        HostCell::global().window_changed();
        Ok(())
    });
    if let Err(status) = result {
        curia::debug!("window change not recorded", { status: status as i32 });
    }
}

/// Game thread, when the overlay stops, before the hooks come off. True when the game must get its own size back
/// now, while a render size is forced or a restore waits: `out_width` x `out_height` is then `client_width` x
/// `client_height`, which the overlay posts as `WM_SIZE`, and the manager forces nothing from then on. False, with
/// nothing written, with nothing due, for a 0 x 0 client size (a minimised window), with no host, or with a null
/// out pointer.
///
/// # Safety
///
/// `out_width` and `out_height` are null or writable for the call.
pub unsafe extern "C" fn amm_host_render_size_shutdown(
    client_width: u32,
    client_height: u32,
    out_width: *mut u32,
    out_height: *mut u32,
) -> bool {
    let result = Boundary::guard(|| {
        let (out_width, out_height) = (Boundary::writable(out_width)?, Boundary::writable(out_height)?);
        let client = RenderSize::new(client_width, client_height);
        let Some(size) = HostCell::global().with(|runtime| Ok(runtime.render_size_shutdown(client)))? else {
            return Ok(false);
        };
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the caller
        // gives
        unsafe {
            out_width.write(size.width);
            out_height.write(size.height);
        }
        Ok(true)
    });
    result.unwrap_or(false)
}
