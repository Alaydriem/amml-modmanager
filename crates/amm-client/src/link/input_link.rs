use std::collections::HashMap;
use std::sync::Arc;

use amm_core::errors::LinkError;
use amm_core::structs::descriptor::{ClaimMode, InputClaim};
use amm_core::structs::lease::AcquireOutcome;
use amml_api::{
    ApiError, ButtonHold, InputBlocks, InputLease, KeyCode, KeyHold, LeaseMode, LeaseRequest, LeaseState, MouseButton,
};

/// The descriptor's `Input` claim as one loader input lease, and the keys and buttons the mod holds through it. The
/// lease is taken by `ManagerLink::acquire(Capability::Input)` and ended by `release`, so the real input it blocks
/// is blocked only while the mod holds `Input`, as with the v2 manager lease.
///
/// It holds state only: the link takes the lease or a hold out of it and calls the loader with no lock held, so an
/// event the loader raises during the call can use the link. A hold or lease taken out is dropped by the caller,
/// holds before the lease.
pub struct InputLink {
    request: Option<LeaseRequest>,
    lease: Option<Arc<InputLease>>,
    keys: HashMap<KeyCode, KeyHold>,
    buttons: Vec<(MouseButton, ButtonHold)>,
}

impl InputLink {
    /// The link of a descriptor with `claim` as its `Input` claim, or of one with no `Input` claim. The passthrough
    /// keys are leaked once, because a lease request holds them as `'static`; a key code above 0xFF is dropped.
    pub fn new(claim: Option<&InputClaim>) -> Self {
        Self { request: claim.map(Self::request_for), lease: None, keys: HashMap::new(), buttons: Vec::new() }
    }

    // `full_block` blocks every real input. Otherwise the lease blocks the real keys (but the passthrough keys) and
    // mouse buttons, and the look unless the claim passes the pointer motion on; `block_player_turn` always blocks
    // the look. This is what the v2 manager kept from the game for an `Input` holder.
    fn request_for(claim: &InputClaim) -> LeaseRequest {
        let passthrough: Vec<KeyCode> = claim
            .passthrough
            .iter()
            .filter_map(|vk| u16::try_from(*vk).ok())
            .filter(|vk| *vk <= 0xFF)
            .map(KeyCode)
            .collect();
        let look = claim.block_player_turn || !claim.pass_pointer_motion;
        let blocks = if claim.full_block {
            InputBlocks::FULL
        } else if look {
            InputBlocks::KEYS | InputBlocks::MOUSE_BUTTONS | InputBlocks::LOOK
        } else {
            InputBlocks::KEYS | InputBlocks::MOUSE_BUTTONS
        };
        let mode = match claim.mode {
            ClaimMode::Exclusive => LeaseMode::Exclusive,
            ClaimMode::Shared => LeaseMode::Shared,
        };
        LeaseRequest { mode, blocks, passthrough: Box::leak(passthrough.into_boxed_slice()), preempt: false }
    }

    /// The request the link gives the loader; `None` for a descriptor with no `Input` claim.
    pub fn request(&self) -> Option<LeaseRequest> {
        self.request
    }

    /// The loader's id of the held lease.
    pub fn lease_id(&self) -> Option<u64> {
        self.lease.as_deref().map(InputLease::id)
    }

    /// The outcome a lease in `state` reports; `None` for a lost lease.
    pub(crate) fn outcome(state: LeaseState) -> Option<AcquireOutcome> {
        match state {
            LeaseState::Active => Some(AcquireOutcome::Granted),
            LeaseState::Suspended => Some(AcquireOutcome::Preempted),
            LeaseState::Lost => None,
        }
    }

    /// The held lease, to call with no lock held.
    pub(crate) fn lease(&self) -> Option<Arc<InputLease>> {
        self.lease.clone()
    }

    /// The held lease, or `Api(Refused)` without one.
    pub(crate) fn lease_or_refused(&self) -> Result<Arc<InputLease>, LinkError> {
        self.lease().ok_or(LinkError::Api(ApiError::Refused))
    }

    /// Keeps `lease`, or gives it back when a lease is held already.
    pub(crate) fn set_lease(&mut self, lease: Arc<InputLease>) -> Option<Arc<InputLease>> {
        if self.lease.is_some() {
            return Some(lease);
        }
        self.lease = Some(lease);
        None
    }

    /// Takes the lease out.
    pub(crate) fn take_lease(&mut self) -> Option<Arc<InputLease>> {
        self.lease.take()
    }

    /// Takes every key and button hold out.
    pub(crate) fn take_holds(&mut self) -> (Vec<KeyHold>, Vec<ButtonHold>) {
        let keys = self.keys.drain().map(|(_, hold)| hold).collect();
        let buttons = self.buttons.drain(..).map(|(_, hold)| hold).collect();
        (keys, buttons)
    }

    pub(crate) fn holds_key(&self, key: KeyCode) -> bool {
        self.keys.contains_key(&key)
    }

    pub(crate) fn take_key(&mut self, key: KeyCode) -> Option<KeyHold> {
        self.keys.remove(&key)
    }

    /// Keeps `hold` of `key`, made through lease `lease`. The hold comes back when that lease is no longer held or the
    /// key is held already, and the caller drops it, which lets the key go.
    pub(crate) fn put_key(&mut self, lease: u64, key: KeyCode, hold: KeyHold) -> Option<KeyHold> {
        if self.lease_id() != Some(lease) || self.keys.contains_key(&key) {
            return Some(hold);
        }
        self.keys.insert(key, hold);
        None
    }

    pub(crate) fn holds_button(&self, button: MouseButton) -> bool {
        self.buttons.iter().any(|(held, _)| *held == button)
    }

    pub(crate) fn take_button(&mut self, button: MouseButton) -> Option<ButtonHold> {
        let index = self.buttons.iter().position(|(held, _)| *held == button)?;
        Some(self.buttons.remove(index).1)
    }

    /// `put_key` for a mouse button.
    pub(crate) fn put_button(&mut self, lease: u64, button: MouseButton, hold: ButtonHold) -> Option<ButtonHold> {
        if self.lease_id() != Some(lease) || self.holds_button(button) {
            return Some(hold);
        }
        self.buttons.push((button, hold));
        None
    }
}
