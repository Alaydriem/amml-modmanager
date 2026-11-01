use egui::{
    Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, ViewportId,
    pos2, vec2,
};

use super::KeyMap;
use crate::structs::frame::{FrameInputData, UiEvent, UiEventKind};

/// Turns one frame's routed events into egui's `RawInput`, converting physical pixels to points.
pub struct RawInputBuilder {
    modifiers: Modifiers,
    held: [bool; 3],
    pointer: Pos2,
    paste_requested: bool,
}

impl RawInputBuilder {
    const VK_SHIFT: [u32; 3] = [0x10, 0xA0, 0xA1];
    const VK_CONTROL: [u32; 3] = [0x11, 0xA2, 0xA3];
    const VK_ALT: [u32; 3] = [0x12, 0xA4, 0xA5];

    pub fn new() -> Self {
        Self { modifiers: Modifiers::NONE, held: [false; 3], pointer: Pos2::ZERO, paste_requested: false }
    }

    /// Fills `out` for this frame. The event vector of `out` is reused.
    pub fn build(&mut self, input: &FrameInputData, out: &mut RawInput) {
        let ppp = input.pixels_per_point.max(f32::EPSILON);
        out.events.clear();
        out.screen_rect =
            Some(Rect::from_min_size(Pos2::ZERO, vec2(input.width as f32 / ppp, input.height as f32 / ppp)));
        out.time = Some(input.time_secs);
        out.focused = true;
        let info = out.viewports.entry(ViewportId::ROOT).or_default();
        info.native_pixels_per_point = Some(ppp);
        info.focused = Some(true);
        for event in input.events {
            self.event(event, ppp, input.input_flags, &mut out.events);
        }
    }

    /// True once after a frame delivered a Ctrl+V key down, which the keyboard must be allowed to reach. egui has
    /// no clipboard of its own, so the caller reads the clipboard and gives the text back as `Event::Paste`.
    pub fn take_paste_request(&mut self) -> bool {
        std::mem::replace(&mut self.paste_requested, false)
    }

    fn event(&mut self, event: &UiEvent, ppp: f32, allowed: u8, out: &mut Vec<Event>) {
        let pointer_ok = allowed & FrameInputData::ALLOW_POINTER != 0;
        let keyboard_ok = allowed & FrameInputData::ALLOW_KEYBOARD != 0;
        match event.kind {
            UiEventKind::PointerMove if pointer_ok => {
                self.pointer = pos2(event.x / ppp, event.y / ppp);
                out.push(Event::PointerMoved(self.pointer));
            }
            UiEventKind::PointerButton if pointer_ok => self.button(event, ppp, out),
            UiEventKind::Wheel if pointer_ok => out.push(Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: vec2(event.x, event.y),
                phase: TouchPhase::Move,
                source: MouseWheelSource::Wheel,
                modifiers: self.modifiers,
            }),
            UiEventKind::Key if keyboard_ok => self.key(event, allowed, out),
            UiEventKind::Char if keyboard_ok => {
                if let Some(c) = char::from_u32(event.ch).filter(|c| !c.is_control()) {
                    out.push(Event::Text(c.to_string()));
                }
            }
            UiEventKind::FocusLost => self.focus_lost(out),
            _ => {}
        }
    }

    fn button(&mut self, event: &UiEvent, ppp: f32, out: &mut Vec<Event>) {
        let Some(button) = Self::pointer_button(event.key) else { return };
        self.pointer = pos2(event.x / ppp, event.y / ppp);
        if let Some(held) = self.held.get_mut(event.key as usize) {
            *held = event.down;
        }
        out.push(Event::PointerButton { pos: self.pointer, button, pressed: event.down, modifiers: self.modifiers });
    }

    fn key(&mut self, event: &UiEvent, allowed: u8, out: &mut Vec<Event>) {
        let vk = event.key;
        if self.modifier(vk, event.down) {
            out.push(Event::ModifiersChanged(self.modifiers));
            return;
        }
        self.modifiers = Self::modifiers_of(event.modifiers);
        let mapped =
            if allowed & FrameInputData::ALLOW_NAV_KEYS != 0 { KeyMap::to_egui(vk) } else { KeyMap::to_egui_safe(vk) };
        let Some(key) = mapped else { return };
        let ctrl_down = self.modifiers.ctrl && event.down;
        self.paste_requested |= key == egui::Key::V && ctrl_down;
        match key {
            egui::Key::C if ctrl_down => out.push(Event::Copy),
            egui::Key::X if ctrl_down => out.push(Event::Cut),
            _ => out.push(Event::Key {
                key,
                physical_key: None,
                pressed: event.down,
                repeat: false,
                modifiers: self.modifiers,
            }),
        }
    }

    fn focus_lost(&mut self, out: &mut Vec<Event>) {
        self.modifiers = Modifiers::NONE;
        out.push(Event::WindowFocused(false));
        for index in 0..self.held.len() {
            if let (true, Some(button)) = (self.held[index], Self::pointer_button(index as u32)) {
                out.push(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                });
            }
            self.held[index] = false;
        }
        out.push(Event::PointerGone);
    }

    fn modifier(&mut self, vk: u32, down: bool) -> bool {
        if Self::VK_SHIFT.contains(&vk) {
            self.modifiers.shift = down;
        } else if Self::VK_CONTROL.contains(&vk) {
            self.modifiers.ctrl = down;
            self.modifiers.command = down;
        } else if Self::VK_ALT.contains(&vk) {
            self.modifiers.alt = down;
        } else {
            return false;
        }
        true
    }

    fn modifiers_of(mask: u8) -> Modifiers {
        let ctrl = mask & UiEvent::MOD_CTRL != 0;
        Modifiers {
            ctrl,
            shift: mask & UiEvent::MOD_SHIFT != 0,
            alt: mask & UiEvent::MOD_ALT != 0,
            command: ctrl,
            ..Modifiers::NONE
        }
    }

    fn pointer_button(index: u32) -> Option<PointerButton> {
        match index {
            0 => Some(PointerButton::Primary),
            1 => Some(PointerButton::Secondary),
            2 => Some(PointerButton::Middle),
            _ => None,
        }
    }
}

impl Default for RawInputBuilder {
    fn default() -> Self {
        Self::new()
    }
}
