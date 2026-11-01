use crate::structs::input::InputEvent;

/// Which keys and pointer buttons are down now, one flag per virtual-key slot.
pub struct KeyState {
    down: [bool; InputEvent::VK_SLOTS],
}

impl KeyState {
    pub fn new() -> Self {
        Self { down: [false; InputEvent::VK_SLOTS] }
    }

    pub fn apply(&mut self, event: &InputEvent) {
        if *event == InputEvent::FocusLost {
            self.clear();
        } else if let Some((slot, down, _)) = event.press() {
            self.down[slot] = down;
        }
    }

    pub fn is_down(&self, vk: u32) -> bool {
        usize::try_from(vk).ok().and_then(|slot| self.down.get(slot)).copied().unwrap_or(false)
    }

    pub fn clear(&mut self) {
        self.down = [false; InputEvent::VK_SLOTS];
    }
}

impl Default for KeyState {
    fn default() -> Self {
        Self::new()
    }
}
