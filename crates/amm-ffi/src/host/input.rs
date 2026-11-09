use amm_abi::convert::pointer_op::PointerOpCodec;
use amm_core::structs::frame::UiEvent;
use amm_core::structs::input::{InputEvent, Modifiers};
use amm_core::structs::render::RenderSize;

use crate::services::{Boundary, HostCell};
use crate::types::{AmmHostFakeKey, AmmHostFakePointer, AmmHostPointerKind, AmmHostRoute};

/// A key from the window or the game. `mods` is a mask: 1 Ctrl, 2 Shift, 4 Alt. `consume` in the answer means the
/// game must not see the key.
pub extern "C" fn amm_host_key(vk: u32, down: bool, repeat: bool, mods: u8, now_ms: u64) -> AmmHostRoute {
    let mods = Modifiers {
        ctrl: mods & UiEvent::MOD_CTRL != 0,
        shift: mods & UiEvent::MOD_SHIFT != 0,
        alt: mods & UiEvent::MOD_ALT != 0,
    };
    let ev = InputEvent::Key { vk, down, repeat, mods };
    Boundary::guard(|| HostCell::global().with(|runtime| Ok(runtime.route(ev, now_ms)))).unwrap_or(AmmHostRoute::PASS)
}

/// A pointer event in physical pixels. `kind` is an `AmmHostPointerKind` value; any other value passes to the
/// game. `button` and `down` are read for `Button`, `wheel` for `Wheel`, and `x`, `y` are the deltas for `Delta`.
pub extern "C" fn amm_host_pointer(kind: u8, x: f32, y: f32, button: u8, down: bool, wheel: f32) -> AmmHostRoute {
    Boundary::guard(|| {
        let ev = match AmmHostPointerKind::try_from(kind)? {
            AmmHostPointerKind::Move => InputEvent::PointerMove { x, y },
            AmmHostPointerKind::Button => InputEvent::PointerButton { button, down, x, y },
            AmmHostPointerKind::Wheel => InputEvent::Wheel { delta: wheel },
            AmmHostPointerKind::Delta => InputEvent::PointerDelta { dx: x, dy: y },
        };
        HostCell::global().with(|runtime| Ok(runtime.route(ev, 0)))
    })
    .unwrap_or(AmmHostRoute::PASS)
}

/// One character of text input, as a Unicode code point.
pub extern "C" fn amm_host_char(ch: u32) -> AmmHostRoute {
    Boundary::guard(|| HostCell::global().with(|runtime| Ok(runtime.route(InputEvent::Char(ch), 0))))
        .unwrap_or(AmmHostRoute::PASS)
}

/// The game window lost focus: every held key and button is released to the mods that got its press. True when
/// the game must not see the focus loss: an exclusive Input holder with `full_block` is active, and it keeps its
/// held keys and fake keys. False with no host.
pub extern "C" fn amm_host_focus_lost() -> bool {
    Boundary::guard(|| HostCell::global().with(|runtime| Ok(runtime.route(InputEvent::FocusLost, 0).consume)))
        .unwrap_or(false)
}

/// Game thread, once per tick. True once after a full block ended that had kept a focus loss from the game, unless
/// the game has seen a focus loss since: the caller then lets the game's window procedure see one focus loss while
/// the window is in the background. False with no host.
pub extern "C" fn amm_host_take_focus_loss_owed() -> bool {
    Boundary::guard(|| HostCell::global().with(|runtime| Ok(runtime.take_focus_loss_owed()))).unwrap_or(false)
}

/// Game thread, once per tick. Moves up to `capacity` fake key presses the mods asked for with `send_key` into
/// `out`, oldest first, and returns how many it wrote; the rest wait for the next call. The caller feeds each press
/// to the game as keyboard input. 0, with nothing written, with no host or with a null or unaligned `out`.
///
/// # Safety
///
/// `out` is null or has room for `capacity` entries, writable for the call.
pub unsafe extern "C" fn amm_host_take_fake_keys(out: *mut AmmHostFakeKey, capacity: usize) -> usize {
    Boundary::guard(|| {
        if capacity == 0 {
            return Ok(0);
        }
        let out = Boundary::writable(out)?;
        HostCell::global().with(|runtime| {
            Ok(runtime.take_fake_keys(capacity, |position, op| {
                // SAFETY: out is non-null and aligned, as checked, and the caller gives room for `capacity`
                // entries; position is below capacity
                unsafe { out.add(position).write(AmmHostFakeKey { vk: op.vk, down: u8::from(op.down) }) }
            }))
        })
    })
    .unwrap_or(0)
}

/// Game thread, once per tick. Moves up to `capacity` fake mouse actions the mods asked for with `send_pointer` into
/// `out`, oldest first, and returns how many it wrote; the rest wait for the next call. The caller feeds each action
/// to the game as mouse input. 0, with nothing written, with no host or with a null or unaligned `out`.
///
/// # Safety
///
/// `out` is null or has room for `capacity` entries, writable for the call.
pub unsafe extern "C" fn amm_host_take_fake_pointer(out: *mut AmmHostFakePointer, capacity: usize) -> usize {
    Boundary::guard(|| {
        if capacity == 0 {
            return Ok(0);
        }
        let out = Boundary::writable(out)?;
        HostCell::global().with(|runtime| {
            Ok(runtime.take_fake_pointer(capacity, |position, op| {
                let (op, value) = PointerOpCodec::encode(op);
                // SAFETY: out is non-null and aligned, as checked, and the caller gives room for `capacity`
                // entries; position is below capacity
                unsafe { out.add(position).write(AmmHostFakePointer { op, value }) }
            }))
        })
    })
    .unwrap_or(0)
}

/// Game thread. Lets go of every fake key and mouse button the mods hold: the releases wait for the next
/// `amm_host_take_fake_keys` and `amm_host_take_fake_pointer`, and each mod that lost one gets `FakeKeysReleased`.
/// Does nothing with no host.
pub extern "C" fn amm_host_release_fake_keys() {
    let _ = Boundary::guard(|| {
        HostCell::global().with(|runtime| {
            runtime.release_fake_keys();
            Ok(())
        })
    });
}

/// Input thread, for each mouse event of the game's own feed. `x`, `y` is the real cursor in back-buffer pixels and
/// `client_width` x `client_height` the window's client size. True when the game must see its mouse at `out_x`,
/// `out_y`, in the pixels of the game's own feed: inside the game viewport of the Overlay holder, the point of the
/// full window it stands for; while a render size is forced, every point, in back-buffer pixels, because the game
/// then sees its window at the back-buffer size. False, with nothing written, otherwise, with no host, or with a
/// null out pointer.
///
/// # Safety
///
/// `out_x` and `out_y` are null or writable for the call.
pub unsafe extern "C" fn amm_host_game_point(
    x: f32,
    y: f32,
    client_width: u32,
    client_height: u32,
    out_x: *mut f32,
    out_y: *mut f32,
) -> bool {
    Boundary::guard(|| {
        let (out_x, out_y) = (Boundary::writable(out_x)?, Boundary::writable(out_y)?);
        let client = RenderSize::new(client_width, client_height);
        let Some((game_x, game_y)) = HostCell::global().with(|runtime| Ok(runtime.game_point((x, y), client)))? else {
            return Ok(false);
        };
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the
        // caller gives
        unsafe {
            out_x.write(game_x);
            out_y.write(game_y);
        }
        Ok(true)
    })
    .unwrap_or(false)
}
