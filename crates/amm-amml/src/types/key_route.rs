/// The answer of `KeyRouter::on_key`: `consume` is true when the game must not see the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyRoute {
    pub consume: bool,
}

impl KeyRoute {
    /// The key reaches the game.
    pub const PASS: KeyRoute = KeyRoute { consume: false };
}
