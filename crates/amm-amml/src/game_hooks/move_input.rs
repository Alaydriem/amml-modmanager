use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// static void ClientInputUpdateSystem::extractRawHIDInput(MovementAbilitiesComponent const&, MoveInputComponent const&,
// ActorDataFlagComponent const&, RawMoveInputComponent&, Optional<SneakingComponent const>,
// Optional<WasInWaterFlagComponent const>): the game fills the raw move input of the player from its keyboard here.
// After the original, the manager writes the held move intent into the component, the fourth argument (r9); every
// argument reaches the original unchanged
amml_detour!(pub mod extract_raw_hid_input(abilities: usize, move_input: usize, actor_flags: usize, raw: usize, sneaking: usize, in_water: usize) -> () |original| {
    original(abilities, move_input, actor_flags, raw, sneaking, in_water);
    if let Some(runtime) = ManagerRuntime::get() {
        runtime.move_input(raw);
    }
});
