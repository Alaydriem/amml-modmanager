use crate::config::{ExclusiveResolution, PairKey, ResolutionBook, SuspensionResolution};
use crate::services::event::EventHub;
use crate::services::registry::ModRegistry;
use crate::structs::descriptor::{Capability, ClaimMode};
use crate::structs::event::ModEvent;
use crate::structs::lease::{AcquireOutcome, CapabilityLeases};
use crate::structs::registry::{ModIndex, ModMask};

/// Decides who holds each capability. State is one `CapabilityLeases` per capability, so it is
/// bounded by the capability count and the mod mask width. Every state change that other mods must
/// see is pushed to their event queues in the order it happens. A resolution lookup builds a
/// `PairKey`, which allocates; leases change on operator action, not per frame.
///
/// Per-key queries must not allocate, so the arbiter keeps fixed tables: each mod's claim mode, and
/// per capability and mod the mods it coexists with and the mods it is preferred over. They are
/// rebuilt by `refresh_blocked` and by every `acquire`. `blocked` is derived from them and the lease
/// state, and is recomputed on every lease change. After a registration, an enable or disable, or a
/// resolution change, the manager calls `refresh_blocked`.
pub struct LeaseArbiter {
    leases: [CapabilityLeases; Capability::ALL.len()],
    claims: [[Option<ClaimMode>; Capability::ALL.len()]; ModMask::CAPACITY],
    coexist: [[ModMask; ModMask::CAPACITY]; Capability::ALL.len()],
    preferred: [[ModMask; ModMask::CAPACITY]; Capability::ALL.len()],
    blocked: [[Option<ModIndex>; Capability::ALL.len()]; ModMask::CAPACITY],
}

impl LeaseArbiter {
    pub fn new() -> Self {
        Self {
            leases: [CapabilityLeases::new(); Capability::ALL.len()],
            claims: [[None; Capability::ALL.len()]; ModMask::CAPACITY],
            coexist: [[ModMask::default(); ModMask::CAPACITY]; Capability::ALL.len()],
            preferred: [[ModMask::default(); ModMask::CAPACITY]; Capability::ALL.len()],
            blocked: [[None; Capability::ALL.len()]; ModMask::CAPACITY],
        }
    }

    /// The exclusive holder that stops `who` from using `cap` now, or `None`. It reads a table, so
    /// it does not allocate. It agrees with `blocked_by` as of the last `refresh_blocked` or `acquire`.
    pub fn blocked(&self, who: ModIndex, cap: Capability) -> Option<ModIndex> {
        self.blocked.get(usize::from(who.0))?[Self::slot(cap)]
    }

    /// Rebuilds the claim and resolution tables from the registry and the book, then the blocked table.
    pub fn refresh_blocked(&mut self, registry: &ModRegistry, book: &ResolutionBook) {
        // With no stored resolution every pair takes the default, so no pair lookup (which allocates a key) is needed
        let defaults_only = book.is_empty();
        for cap in Capability::ALL {
            let slot = Self::slot(cap);
            for a in 0..ModMask::CAPACITY as u8 {
                let who = ModIndex(a);
                self.claims[usize::from(a)][slot] = Self::claim_mode(registry, who, cap);
                let mut coexist = ModMask::default();
                let mut preferred = ModMask::default();
                for b in 0..ModMask::CAPACITY as u8 {
                    let other = ModIndex(b);
                    if defaults_only || a == b || self.claims[usize::from(a)][slot].is_none() {
                        continue;
                    }
                    if Self::coexists(registry, book, cap, who, other) {
                        coexist.insert(other);
                    }
                    if Self::prefers(registry, book, cap, who, other) {
                        preferred.insert(other);
                    }
                }
                self.coexist[slot][usize::from(a)] = coexist;
                self.preferred[slot][usize::from(a)] = preferred;
            }
        }
        self.recompute_blocked();
    }

    fn recompute_blocked(&mut self) {
        for cap in Capability::ALL {
            let slot = Self::slot(cap);
            let exclusive = self.leases[slot].exclusive;
            for who in 0..ModMask::CAPACITY {
                let index = ModIndex(who as u8);
                let blocked =
                    exclusive.filter(|holder| *holder != index).filter(|holder| match self.claims[who][slot] {
                        Some(ClaimMode::Shared) => !self.coexist[slot][who].contains(*holder),
                        Some(ClaimMode::Exclusive) => !self.preferred[slot][who].contains(*holder),
                        None => false,
                    });
                self.blocked[who][slot] = blocked;
            }
        }
    }

    /// Asks for a lease on `cap`. The mode (exclusive or shared) comes from the caller's claim.
    /// A capability the caller did not claim is a caller bug: the call returns
    /// `Denied { holder: who }` and changes nothing.
    pub fn acquire(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        who: ModIndex,
        cap: Capability,
        hub: &mut EventHub,
    ) -> AcquireOutcome {
        if Self::claim_mode(registry, who, cap).is_none() {
            return AcquireOutcome::Denied { holder: who };
        }
        self.refresh_blocked(registry, book);
        let outcome = self.acquire_claimed(registry, book, who, cap, hub);
        self.recompute_blocked();
        outcome
    }

    fn acquire_claimed(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        who: ModIndex,
        cap: Capability,
        hub: &mut EventHub,
    ) -> AcquireOutcome {
        let Some(mode) = Self::claim_mode(registry, who, cap) else {
            return AcquireOutcome::Denied { holder: who };
        };
        let state = &mut self.leases[Self::slot(cap)];
        if state.suspended.contains(who)
            && let Some(holder) = state.exclusive
        {
            state.denied_since_grant.insert(who);
            return AcquireOutcome::Denied { holder };
        }
        if state.exclusive == Some(who) || state.shared.contains(who) {
            return AcquireOutcome::Granted;
        }
        match mode {
            ClaimMode::Exclusive => self.acquire_exclusive(registry, book, who, cap, hub),
            ClaimMode::Shared => self.acquire_shared(registry, book, who, cap, hub),
        }
    }

    pub fn release(&mut self, who: ModIndex, cap: Capability, hub: &mut EventHub) {
        self.release_lease(who, cap, hub);
        self.recompute_blocked();
    }

    fn release_lease(&mut self, who: ModIndex, cap: Capability, hub: &mut EventHub) {
        let state = &mut self.leases[Self::slot(cap)];
        state.denied_since_grant.remove(who);
        if state.exclusive == Some(who) {
            state.exclusive = None;
            state.forget(who);
            let resumed = state.suspended;
            let mut available = state.denied_since_grant;
            for mod_index in resumed.iter() {
                available.remove(mod_index);
            }
            state.suspended = ModMask::default();
            state.denied_since_grant = ModMask::default();
            for mod_index in resumed.iter() {
                hub.push(mod_index, ModEvent::Resumed(cap));
            }
            for mod_index in available.iter() {
                hub.push(mod_index, ModEvent::CapabilityAvailable(cap));
            }
        } else if state.shared.contains(who) {
            state.shared.remove(who);
            state.suspended.remove(who);
            state.forget(who);
        }
    }

    pub fn release_all(&mut self, who: ModIndex, hub: &mut EventHub) {
        for cap in Capability::ALL {
            self.release_lease(who, cap, hub);
        }
        self.recompute_blocked();
    }

    /// The mods that hold a lease on `cap` and are not suspended.
    pub fn holders(&self, cap: Capability) -> ModMask {
        self.leases[Self::slot(cap)].active()
    }

    pub fn exclusive_holder(&self, cap: Capability) -> Option<ModIndex> {
        self.leases[Self::slot(cap)].exclusive
    }

    /// True when `who` holds a lease on `cap` and is not suspended.
    pub fn holds(&self, who: ModIndex, cap: Capability) -> bool {
        self.holders(cap).contains(who)
    }

    /// True when a mod other than `who` holds a lease on `cap` and is not suspended.
    pub fn held_by_other(&self, who: ModIndex, cap: Capability) -> bool {
        let mut others = self.holders(cap);
        others.remove(who);
        !others.is_empty()
    }

    /// True when `who` holds a shared lease on `cap` that an exclusive lease suspends.
    pub fn is_suspended(&self, who: ModIndex, cap: Capability) -> bool {
        self.leases[Self::slot(cap)].suspended.contains(who)
    }

    /// The exclusive holder that stops `who` from using `cap` now, or `None`. It reads state and
    /// changes nothing. It also names the holder for a mod that is suspended.
    pub fn blocked_by(
        &self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        who: ModIndex,
        cap: Capability,
    ) -> Option<ModIndex> {
        let holder = self.exclusive_holder(cap).filter(|holder| *holder != who)?;
        let blocks = match Self::claim_mode(registry, who, cap)? {
            ClaimMode::Shared => !Self::coexists(registry, book, cap, who, holder),
            ClaimMode::Exclusive => !Self::prefers(registry, book, cap, who, holder),
        };
        blocks.then_some(holder)
    }

    /// The active holders of `cap`, the newest acquisition first.
    pub fn most_recent(&self, cap: Capability) -> impl Iterator<Item = ModIndex> + '_ {
        let state = &self.leases[Self::slot(cap)];
        let active = state.active();
        state
            .acquired_order
            .iter()
            .rev()
            .filter(|slot| **slot != CapabilityLeases::EMPTY)
            .map(|slot| ModIndex(*slot))
            .filter(move |index| active.contains(*index))
    }

    fn acquire_exclusive(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        who: ModIndex,
        cap: Capability,
        hub: &mut EventHub,
    ) -> AcquireOutcome {
        let slot = Self::slot(cap);
        let mut outcome = AcquireOutcome::Granted;
        if let Some(holder) = self.leases[slot].exclusive {
            if !Self::prefers(registry, book, cap, who, holder) {
                self.leases[slot].denied_since_grant.insert(who);
                return AcquireOutcome::Denied { holder };
            }
            hub.push(holder, ModEvent::Revoked { capability: cap, by: who });
            self.leases[slot].exclusive = None;
            self.leases[slot].forget(holder);
            outcome = AcquireOutcome::Preempted;
        }
        self.reconcile_shared(registry, book, who, cap, hub, outcome == AcquireOutcome::Preempted);
        let state = &mut self.leases[slot];
        state.exclusive = Some(who);
        state.denied_since_grant.remove(who);
        state.touch(who);
        hub.push(who, ModEvent::Granted(cap));
        outcome
    }

    fn acquire_shared(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        who: ModIndex,
        cap: Capability,
        hub: &mut EventHub,
    ) -> AcquireOutcome {
        let state = &mut self.leases[Self::slot(cap)];
        if let Some(holder) = state.exclusive
            && !Self::coexists(registry, book, cap, who, holder)
        {
            state.denied_since_grant.insert(who);
            return AcquireOutcome::Denied { holder };
        }
        state.shared.insert(who);
        state.denied_since_grant.remove(who);
        state.touch(who);
        hub.push(who, ModEvent::Granted(cap));
        AcquireOutcome::Granted
    }

    // Brings the suspended set in line with the resolution for each shared holder against the new
    // exclusive holder `by`. `announce` repeats `Suspended` for holders that stay suspended, so they
    // learn the new holder.
    fn reconcile_shared(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        by: ModIndex,
        cap: Capability,
        hub: &mut EventHub,
        announce: bool,
    ) {
        let shared = self.leases[Self::slot(cap)].shared;
        for holder in shared.iter() {
            let should_suspend = !Self::coexists(registry, book, cap, holder, by);
            let state = &mut self.leases[Self::slot(cap)];
            let was_suspended = state.suspended.contains(holder);
            if should_suspend {
                state.suspended.insert(holder);
                if !was_suspended || announce {
                    hub.push(holder, ModEvent::Suspended { capability: cap, by });
                }
            } else if was_suspended {
                state.suspended.remove(holder);
                hub.push(holder, ModEvent::Resumed(cap));
            }
        }
    }

    fn slot(cap: Capability) -> usize {
        cap as usize
    }

    fn claim_mode(registry: &ModRegistry, who: ModIndex, cap: Capability) -> Option<ClaimMode> {
        let registered = registry.get(who)?;
        registered.descriptor.claims.iter().find(|claim| claim.capability == cap).map(|claim| claim.mode)
    }

    // True when the stored suspension resolution for the pair is Coexist. The default is Suspend.
    fn coexists(registry: &ModRegistry, book: &ResolutionBook, cap: Capability, a: ModIndex, b: ModIndex) -> bool {
        let (Some(first), Some(second)) = (registry.get(a), registry.get(b)) else {
            return false;
        };
        let key = PairKey::new(cap, &first.descriptor.id(), &second.descriptor.id());
        book.suspension_for(&key) == Some(&SuspensionResolution::Coexist)
    }

    // True when the stored exclusive resolution for the pair prefers `who` over `holder`.
    // The default is DenySecond, which prefers nobody. No mod is preferred for RenderSize: taking it from its
    // holder would change the game's size in the middle of the holder's work.
    fn prefers(
        registry: &ModRegistry,
        book: &ResolutionBook,
        cap: Capability,
        who: ModIndex,
        holder: ModIndex,
    ) -> bool {
        if cap == Capability::RenderSize {
            return false;
        }
        let (Some(caller), Some(current)) = (registry.get(who), registry.get(holder)) else {
            return false;
        };
        let key = PairKey::new(cap, &caller.descriptor.id(), &current.descriptor.id());
        matches!(book.exclusive_for(&key), Some(ExclusiveResolution::Prefer(id)) if *id == caller.descriptor.id())
    }
}

impl Default for LeaseArbiter {
    fn default() -> Self {
        Self::new()
    }
}
