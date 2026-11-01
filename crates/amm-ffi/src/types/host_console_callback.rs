/// The callback that shows one line in the LeviLamina console: an `AmmLogLevel` value and UTF-8 text valid for
/// the call.
pub type AmmHostConsoleCallback = unsafe extern "C" fn(level: u8, msg: *const u8, len: usize);
