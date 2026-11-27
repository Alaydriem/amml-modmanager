use amml_api::testing::FakeGame;

use crate::link::ManagerLink;

/// A link connected to the fake manager tables, and the fake loader it runs on. The link is dropped before the
/// loader, so its listeners and lease end while the loader still lives.
pub struct FakeConnection {
    pub link: ManagerLink,
    pub game: FakeGame,
}
