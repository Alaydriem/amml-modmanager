use crate::structs::input::Modifiers;

/// One input event from the window or the game, before routing. Plain data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    Key { vk: u32, down: bool, repeat: bool, mods: Modifiers },
    PointerMove { x: f32, y: f32 },
    PointerButton { button: u8, down: bool, x: f32, y: f32 },
    PointerDelta { dx: f32, dy: f32 },
    Wheel { delta: f32 },
    Char(u32),
    FocusLost,
}

impl InputEvent {
    /// The number of virtual-key slots that key state is kept for.
    pub const VK_SLOTS: usize = 256;

    // The virtual-key codes of pointer buttons 0 to 4: left, right, middle, X1, X2.
    const BUTTON_VKS: [u32; 5] = [0x01, 0x02, 0x04, 0x05, 0x06];

    /// The pointer button whose virtual-key code is `vk`, or `None` for a keyboard key.
    pub fn button_for_vk(vk: u32) -> Option<u8> {
        Self::BUTTON_VKS.iter().position(|button_vk| *button_vk == vk).and_then(|button| u8::try_from(button).ok())
    }

    /// The key-state slot, down flag and repeat flag of a key or pointer button event. Pointer
    /// buttons use their virtual-key codes. Events outside the slot range give `None`.
    pub fn press(&self) -> Option<(usize, bool, bool)> {
        let (vk, down, repeat) = match *self {
            InputEvent::Key { vk, down, repeat, .. } => (vk, down, repeat),
            InputEvent::PointerButton { button, down, .. } => {
                (*Self::BUTTON_VKS.get(usize::from(button))?, down, false)
            }
            _ => return None,
        };
        let slot = usize::try_from(vk).ok().filter(|slot| *slot < Self::VK_SLOTS)?;
        Some((slot, down, repeat))
    }
}
