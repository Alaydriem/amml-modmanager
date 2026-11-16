//! The game functions the manager hooks. Each body hands the raw arguments to `ManagerRuntime` and calls the
//! original unless the manager cancels the call; bool and char arguments and results cross as bytes, so no bool
//! invariant is assumed of the game's registers.

pub mod gamepad;
pub mod grab;
pub mod keys;
pub mod mouse;
pub mod move_input;
pub mod tick;
pub mod turn;
pub mod world;

pub use gamepad::gamepad_refresh;
pub use grab::grab_mouse;
pub use keys::{controller_update, key_down, key_up};
pub use mouse::feed;
pub use move_input::extract_raw_hid_input;
pub use tick::client_update;
pub use turn::apply_turn_delta;
pub use world::{level_exit, player_loaded};
