//! The manager's five registry tables as fakes for client tests, provided into an `amml_api::testing::FakeGame`.
//! Their state is kept per thread and Curia is installed once per process, so run client tests with
//! `cargo nextest` (one process per test), and make every table call from the test thread.

pub mod fake_connection;
pub mod fake_state;
pub mod fake_table;
pub mod setting_call;

pub use fake_connection::FakeConnection;
pub use fake_state::FakeState;
pub use fake_table::FakeTable;
pub use setting_call::SettingCall;
