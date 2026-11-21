use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_abi::types::{AmmCallbacks, AmmFrameInput, AmmFrameOutput, AmmStatus};

// counters of the one consumer a test registers; nextest gives each test its own process
static FRAMES: AtomicU32 = AtomicU32::new(0);
static TEXTURES_LOST: AtomicU32 = AtomicU32::new(0);

/// The render-thread callbacks of a consumer mod that draws an empty layer and counts what the manager asks of it.
pub struct ConsumerCallbacks;

impl ConsumerCallbacks {
    pub fn table() -> AmmCallbacks {
        AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: std::ptr::null_mut(),
            frame: Some(Self::frame),
            textures_lost: Some(Self::textures_lost),
        }
    }

    /// How many frames the manager asked the consumer for.
    pub fn frames() -> u32 {
        FRAMES.load(Ordering::SeqCst)
    }

    /// How many times the manager told the consumer its textures are lost.
    pub fn textures_lost_calls() -> u32 {
        TEXTURES_LOST.load(Ordering::SeqCst)
    }

    unsafe extern "C" fn frame(_user: *mut c_void, _input: *const AmmFrameInput, _out: *mut AmmFrameOutput) -> i32 {
        FRAMES.fetch_add(1, Ordering::SeqCst);
        AmmStatus::Ok as i32
    }

    unsafe extern "C" fn textures_lost(_user: *mut c_void) {
        TEXTURES_LOST.fetch_add(1, Ordering::SeqCst);
    }
}
