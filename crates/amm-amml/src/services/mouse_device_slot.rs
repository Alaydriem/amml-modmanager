use crate::traits::HookHost;

/// The game's one `MouseDevice`, a static global the `MouseDevice.instance` entry gives, read at enable. With it,
/// fake mouse buttons reach the game before the first real mouse event. The entry is optional: without it the
/// device comes from the first real mouse event, as before, with one log line.
pub struct MouseDeviceSlot;

impl MouseDeviceSlot {
    pub const ENTRY: &'static str = "MouseDevice.instance";
    const FEATURE: &'static str = "fake mouse buttons before the first real mouse event";

    /// The address of the device when the entry resolved to one; otherwise `None`, with one warning.
    pub fn resolve<H: HookHost>(host: &H) -> Option<usize> {
        match host.resolve(Self::ENTRY) {
            Ok(0) => {
                host.warn(&format!("{} resolved to 0; {} is off", Self::ENTRY, Self::FEATURE));
                None
            }
            Ok(address) => match usize::try_from(address) {
                Ok(address) => Some(address),
                Err(_) => {
                    host.warn(&format!(
                        "{} is {address:#x}, past the address size; {} is off",
                        Self::ENTRY,
                        Self::FEATURE
                    ));
                    None
                }
            },
            Err(error) => {
                host.warn(&format!("{} did not resolve ({error}); {} is off", Self::ENTRY, Self::FEATURE));
                None
            }
        }
    }
}
