use std::collections::VecDeque;

use amm_abi::types::AmmHandle;
use amm_core::errors::LinkError;
use amm_core::structs::event::ModEvent;
use amml_api::Ctx;

use super::{FrameSlot, InputLink, LinkState, ManagerTables};

/// The state a `ManagerLink` shares with its listeners, behind the link's lock. It holds state only: `LinkCell`
/// changes it in short steps and makes every loader and manager call with the lock released.
pub(crate) struct LinkShared {
    pub(crate) ctx: Ctx,
    pub(crate) descriptor: &'static str,
    pub(crate) state: LinkState,
    // A connect runs with the lock released; an event during it sees `Waiting` and starts no second connect
    pub(crate) connecting: bool,
    pub(crate) fault: Option<LinkError>,
    pub(crate) connection: Option<(ManagerTables, AmmHandle)>,
    pub(crate) input: InputLink,
    pub(crate) frame: Option<FrameSlot>,
    // At most `MAX_PENDING` events; a new event past it evicts the oldest
    pub(crate) pending: VecDeque<ModEvent>,
}

impl LinkShared {
    /// The most events the link keeps for a mod that does not read them; the oldest is dropped first.
    pub(crate) const MAX_PENDING: usize = 256;

    pub(crate) fn new(ctx: Ctx, descriptor: &'static str, input: InputLink, frame: Option<FrameSlot>) -> Self {
        Self {
            ctx,
            descriptor,
            state: LinkState::Waiting,
            connecting: false,
            fault: None,
            connection: None,
            input,
            frame,
            pending: VecDeque::new(),
        }
    }

    /// Keeps `event` for `events()`. Past `MAX_PENDING` the oldest event is dropped and logged.
    pub(crate) fn push_pending(&mut self, event: ModEvent) {
        if self.pending.len() >= Self::MAX_PENDING
            && let Some(dropped) = self.pending.pop_front()
        {
            let kind = format!("{dropped:?}");
            self.ctx.log().warn("the mod reads no events, the oldest unread event is dropped", &[("event", &kind)]);
        }
        self.pending.push_back(event);
    }

    /// The tables and handle while connected; `NotConnected` (or the kept fault) while waiting, `Gone` after.
    pub(crate) fn connection(&self) -> Result<(ManagerTables, AmmHandle), LinkError> {
        match (self.state, self.connection) {
            (LinkState::Connected, Some(connection)) => Ok(connection),
            (LinkState::Gone, _) => Err(LinkError::Gone),
            _ => Err(self.fault.clone().unwrap_or(LinkError::NotConnected)),
        }
    }

    /// The input link, once connected.
    pub(crate) fn input_mut(&mut self) -> Result<&mut InputLink, LinkError> {
        self.connection()?;
        Ok(&mut self.input)
    }

    /// Whether a connect may start now.
    pub(crate) fn may_connect(&self) -> bool {
        self.state == LinkState::Waiting && !self.connecting && self.fault.is_none()
    }
}
