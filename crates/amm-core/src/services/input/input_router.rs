use crate::services::input::ChordMatcher;
use crate::structs::descriptor::{Chord, KeyScope, KeybindDecl};
use crate::structs::event::KeybindPhase;
use crate::structs::input::{InputEvent, RouteContext, RouteDecision};
use crate::structs::registry::{ModIndex, ModMask};
use crate::structs::toast::ToastKind;

/// Decides where each input event goes. State is fixed tables over the virtual-key slots: whether
/// each key is down, whether its down was kept from the game, the mods that got it as a raw event
/// and the keybind it pressed. A key-up and a repeat follow the down, so a release reaches the mod
/// that got the press even after that mod lost its lease, and a key pressed while the panel was
/// open never reaches a mod. A press on the manager's menu button goes to the manager's pass, and so does
/// its release. While a mod has a focused text field, that mod gets every key press as a raw event and no
/// keybind runs, except the manager toggle chord and the open panel. F3 is a modifier the router tracks
/// itself: while it is down, every other key's chord has `f3` set, and F3 and any key with no F3 bind are kept
/// from the game and from every mod. Game input leases are the loader's, so no mod gets keys here as an input holder.
/// `route` does not allocate.
pub struct InputRouter {
    slots: ModMask,
    pressed: [bool; InputEvent::VK_SLOTS],
    swallowed: [bool; InputEvent::VK_SLOTS],
    panel_down: [bool; InputEvent::VK_SLOTS],
    raw_down: [ModMask; InputEvent::VK_SLOTS],
    bind_down: [Option<(ModIndex, u16)>; InputEvent::VK_SLOTS],
}

impl InputRouter {
    pub fn new(max_mods: usize) -> Self {
        let slots = match max_mods {
            0 => ModMask::default(),
            n if n >= ModMask::CAPACITY => ModMask(u32::MAX),
            n => ModMask((1u32 << n) - 1),
        };
        Self {
            slots,
            pressed: [false; InputEvent::VK_SLOTS],
            swallowed: [false; InputEvent::VK_SLOTS],
            panel_down: [false; InputEvent::VK_SLOTS],
            raw_down: [ModMask::default(); InputEvent::VK_SLOTS],
            bind_down: [None; InputEvent::VK_SLOTS],
        }
    }

    pub fn route(&mut self, ev: &InputEvent, ctx: &RouteContext, matcher: &ChordMatcher) -> RouteDecision {
        match *ev {
            InputEvent::FocusLost => {
                self.clear();
                RouteDecision::default()
            }
            InputEvent::Key { vk, mods, .. } => match ev.press() {
                Some(press) => {
                    let chord = mods.chord_for(vk).with_f3(self.f3_held());
                    self.route_press(press, Some(chord), ctx, matcher)
                }
                None => Self::untracked(ctx),
            },
            InputEvent::PointerButton { .. } => match ev.press() {
                Some(press) => self.route_press(press, None, ctx, matcher),
                None => self.route_pointer(ctx, true, true),
            },
            InputEvent::PointerDelta { .. } => self.route_pointer(ctx, true, false),
            InputEvent::Wheel { .. } => self.route_pointer(ctx, true, true),
            InputEvent::PointerMove { .. } => self.route_pointer(ctx, false, true),
            InputEvent::Char(_) => self.route_char(ctx),
        }
    }

    /// The keybind that the down of `vk` pressed and whose release is still owed.
    pub fn held_keybind(&self, vk: u32) -> Option<(ModIndex, u16)> {
        self.bind_down[Self::vk_slot(vk)?]
    }

    /// Whether keybind `action` of the mod in `index` is held down: its press arrived and its release has not.
    pub fn bind_held(&self, index: ModIndex, action: u16) -> bool {
        self.bind_down.contains(&Some((index, action)))
    }

    /// Drops every release owed to a mod that left, so a new mod in its slot gets none of them.
    pub fn forget(&mut self, index: ModIndex) {
        for targets in self.raw_down.iter_mut() {
            targets.remove(index);
        }
        for bind in self.bind_down.iter_mut() {
            if bind.is_some_and(|(owner, _)| owner == index) {
                *bind = None;
            }
        }
    }

    fn f3_held(&self) -> bool {
        self.pressed[Chord::F3 as usize]
    }

    fn vk_slot(vk: u32) -> Option<usize> {
        usize::try_from(vk).ok().filter(|slot| *slot < InputEvent::VK_SLOTS)
    }

    fn route_press(
        &mut self,
        (slot, down, repeat): (usize, bool, bool),
        chord: Option<Chord>,
        ctx: &RouteContext,
        matcher: &ChordMatcher,
    ) -> RouteDecision {
        if !down {
            return self.release(slot, ctx.panel_open);
        }
        if repeat && self.pressed[slot] {
            return self.repeat(slot, ctx.panel_open);
        }
        self.clear_slot(slot);
        self.pressed[slot] = true;
        let decision = if chord == Some(ctx.toggle_chord) {
            RouteDecision { consume_from_game: true, toggle_panel: true, ..RouteDecision::default() }
        } else if ctx.panel_open {
            Self::panel()
        } else if chord.is_none() && ctx.over_menu_button {
            self.panel_down[slot] = true;
            Self::panel()
        } else {
            self.dispatch(slot, chord, ctx, matcher)
        };
        self.swallowed[slot] = decision.consume_from_game;
        decision
    }

    // A release goes where the press went. It is kept from the game only when the press was, so the
    // game never sees an up without its down and never keeps a key stuck down.
    fn release(&mut self, slot: usize, panel_open: bool) -> RouteDecision {
        let decision = RouteDecision {
            consume_from_game: self.swallowed[slot],
            to_panel: panel_open || self.panel_down[slot],
            keybind: self.bind_down[slot].map(|(index, action)| (index, action, KeybindPhase::Released)),
            raw_targets: self.raw_down[slot],
            ..RouteDecision::default()
        };
        self.clear_slot(slot);
        decision
    }

    fn repeat(&self, slot: usize, panel_open: bool) -> RouteDecision {
        if panel_open {
            return Self::panel();
        }
        RouteDecision {
            consume_from_game: self.swallowed[slot],
            keybind: self.bind_down[slot].map(|(index, action)| (index, action, KeybindPhase::Repeat)),
            raw_targets: self.raw_down[slot],
            ..RouteDecision::default()
        }
    }

    // The mod with a focused text field first (for keys), then live keybinds, then the pointer owner (for buttons).
    fn dispatch(
        &mut self,
        slot: usize,
        chord: Option<Chord>,
        ctx: &RouteContext,
        matcher: &ChordMatcher,
    ) -> RouteDecision {
        if chord.is_some()
            && let Some(target) = ctx.keyboard_owner.map(|index| self.single(index)).filter(|mask| !mask.is_empty())
        {
            self.raw_down[slot] = target;
            return RouteDecision { consume_from_game: true, raw_targets: target, ..RouteDecision::default() };
        }
        if let Some(decision) = chord.and_then(|chord| self.keybind(slot, chord, ctx, matcher)) {
            return decision;
        }
        if chord.is_some_and(|chord| chord.f3 || chord.key == Chord::F3) {
            return RouteDecision { consume_from_game: true, ..RouteDecision::default() };
        }
        let owner = ctx.pointer_owner.filter(|_| chord.is_none());
        match owner.map(|index| self.single(index)).filter(|mask| !mask.is_empty()) {
            Some(target) => {
                self.raw_down[slot] = target;
                let consume = !ctx.over_game_viewport;
                RouteDecision { consume_from_game: consume, raw_targets: target, ..RouteDecision::default() }
            }
            None => RouteDecision::default(),
        }
    }

    // The first live keybind on the chord wins. A chord whose only binds are blocked is kept from
    // the game and names the first blocking holder.
    fn keybind(
        &mut self,
        slot: usize,
        chord: Chord,
        ctx: &RouteContext,
        matcher: &ChordMatcher,
    ) -> Option<RouteDecision> {
        let mut blocked = None;
        for (index, action) in matcher.find(chord) {
            let Some(decl) = Self::decl(ctx, index, action) else {
                continue;
            };
            if let Some(toast) = Self::blocked_toast(ctx, index, action, decl) {
                blocked.get_or_insert(toast);
            } else if Self::is_live(ctx, index, decl) {
                self.bind_down[slot] = Some((index, action));
                let keybind = Some((index, action, KeybindPhase::Pressed));
                return Some(RouteDecision { consume_from_game: true, keybind, ..RouteDecision::default() });
            }
        }
        blocked.map(|toast| RouteDecision { consume_from_game: true, blocked: Some(toast), ..RouteDecision::default() })
    }

    fn decl<'a>(ctx: &RouteContext<'a>, index: ModIndex, action: u16) -> Option<&'a KeybindDecl> {
        let registered = ctx.registry.get(index).filter(|registered| registered.enabled)?;
        registered.descriptor.keybinds.get(usize::from(action))
    }

    fn blocked_toast(ctx: &RouteContext, index: ModIndex, action: u16, decl: &KeybindDecl) -> Option<ToastKind> {
        if decl.scope != KeyScope::Global {
            return None;
        }
        let holder = ctx.arbiter.blocked(index, decl.capability)?;
        Some(ToastKind::Blocked { capability: decl.capability, holder, mod_index: index, action })
    }

    fn is_live(ctx: &RouteContext, index: ModIndex, decl: &KeybindDecl) -> bool {
        match decl.scope {
            KeyScope::Global => true,
            KeyScope::WhileHeld => ctx.arbiter.holds(index, decl.capability),
        }
    }

    fn route_pointer(&self, ctx: &RouteContext, raw: bool, to_owner: bool) -> RouteDecision {
        if ctx.panel_open {
            return Self::panel();
        }
        // the menu button is the top layer and shows only while the game shows its cursor; a move over it
        // still reaches the game
        if to_owner && ctx.over_menu_button {
            return RouteDecision { consume_from_game: raw, to_panel: true, ..RouteDecision::default() };
        }
        // a game screen shown in the game viewport is used through it: the game gets the pointer too
        let consume = raw && !ctx.over_game_viewport;
        match ctx.pointer_owner.filter(|_| to_owner).map(|index| self.single(index)) {
            Some(target) if !target.is_empty() => {
                RouteDecision { consume_from_game: consume, raw_targets: target, ..RouteDecision::default() }
            }
            _ => RouteDecision::default(),
        }
    }

    fn route_char(&self, ctx: &RouteContext) -> RouteDecision {
        if ctx.panel_open {
            return Self::panel();
        }
        match ctx.keyboard_owner.map(|index| self.single(index)) {
            Some(target) if !target.is_empty() => {
                RouteDecision { consume_from_game: true, raw_targets: target, ..RouteDecision::default() }
            }
            _ => RouteDecision::default(),
        }
    }

    fn untracked(ctx: &RouteContext) -> RouteDecision {
        if ctx.panel_open { Self::panel() } else { RouteDecision::default() }
    }

    fn panel() -> RouteDecision {
        RouteDecision { consume_from_game: true, to_panel: true, ..RouteDecision::default() }
    }

    fn single(&self, index: ModIndex) -> ModMask {
        let mut mask = ModMask::default();
        mask.insert(index);
        ModMask(mask.0 & self.slots.0)
    }

    fn clear_slot(&mut self, slot: usize) {
        self.pressed[slot] = false;
        self.swallowed[slot] = false;
        self.panel_down[slot] = false;
        self.raw_down[slot] = ModMask::default();
        self.bind_down[slot] = None;
    }

    /// Forgets every held key, as a focus loss does.
    pub(crate) fn clear(&mut self) {
        self.pressed = [false; InputEvent::VK_SLOTS];
        self.swallowed = [false; InputEvent::VK_SLOTS];
        self.panel_down = [false; InputEvent::VK_SLOTS];
        self.raw_down = [ModMask::default(); InputEvent::VK_SLOTS];
        self.bind_down = [None; InputEvent::VK_SLOTS];
    }
}
