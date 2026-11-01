//! A fake manager table for client tests. Its state is kept per thread and Curia is installed once per process,
//! so run client tests with `cargo nextest` (one process per test), and make every table call from the test thread.

pub mod fake_state;
pub mod fake_table;
pub mod logged_call;
pub mod setting_call;

pub use fake_state::FakeState;
pub use fake_table::FakeTable;
pub use logged_call::LoggedCall;
pub use setting_call::SettingCall;
