use amm_client::link::{InputLink, PointerOp};
use amm_client::test_support::FakeTable;
use amm_core::errors::LinkError;
use amm_core::structs::descriptor::{Capability, Descriptor, InputClaim};
use amm_core::structs::lease::AcquireOutcome;
use amml_api::{ApiError, InputBlocks, KeyCode, LeaseMode, Look, MouseButton, MoveIntent};

const FULL_BLOCK: &str = "namespace = \"test\"\nname = \"alaydriem-headless\"\ninput_passthrough = [27, 300]\n\
display_name = \"Headless\"\nversion = \"0.1.0\"\nauthor = \"Alaydriem\"\nconfig_path = \"config.hcl\"\n\
claim \"Input\" {\n  mode       = \"Exclusive\"\n  reason     = \"drives the player\"\n  full_block = true\n}\n";

const LOOK_PASSES: &str = "namespace = \"test\"\nname = \"alaydriem-freecam\"\n\
display_name = \"Freecam\"\nversion = \"0.1.0\"\nauthor = \"Alaydriem\"\nconfig_path = \"config.hcl\"\n\
claim \"Input\" {\n  mode                = \"Shared\"\n  reason              = \"flies the camera\"\n  \
pass_pointer_motion = true\n}\n";

fn claim_of(text: &str) -> InputClaim {
    let descriptor: Descriptor = match hcl::from_str(text) {
        Ok(descriptor) => descriptor,
        Err(error) => panic!("descriptor did not parse: {error}"),
    };
    let Some(claim) = descriptor.input_claim() else { panic!("no Input claim") };
    claim
}

#[test]
fn the_input_claim_becomes_one_lease() {
    let connection = FakeTable::link_without_frame_with(FULL_BLOCK);

    let first = connection.link.acquire(Capability::Input);
    let lease = connection.link.input_lease_id();
    let second = connection.link.acquire(Capability::Input);

    assert_eq!((first, second), (Ok(AcquireOutcome::Granted), Ok(AcquireOutcome::Granted)));
    assert!(lease.is_some());
    assert_eq!(connection.link.input_lease_id(), lease, "a second acquire keeps the one lease");
    assert!(!connection.game.real_key(KeyCode::W.0, true), "FULL blocks a real key");
    assert!(connection.game.real_key(KeyCode::ESCAPE.0, true), "the passthrough key reaches the game");
    connection.link.release(Capability::Input);
    assert_eq!(connection.link.input_lease_id(), None);
    assert!(connection.game.real_key(KeyCode::D.0, true), "nothing is blocked after the release");
}

#[test]
fn the_request_follows_the_claim_flags() {
    let full = InputLink::new(Some(&claim_of(FULL_BLOCK))).request();
    let look_passes = InputLink::new(Some(&claim_of(LOOK_PASSES))).request();
    let none = InputLink::new(None).request();

    let Some(full) = full else { panic!("no request for the full block claim") };
    assert_eq!(full.mode, LeaseMode::Exclusive);
    assert_eq!(full.blocks, InputBlocks::FULL);
    assert_eq!(full.passthrough, &[KeyCode::ESCAPE][..], "a code above 0xFF is dropped");
    assert!(!full.preempt);
    let Some(look_passes) = look_passes else { panic!("no request for the shared claim") };
    assert_eq!(look_passes.mode, LeaseMode::Shared);
    assert_eq!(look_passes.blocks, InputBlocks::KEYS | InputBlocks::MOUSE_BUTTONS);
    assert!(none.is_none());
}

#[test]
fn send_key_holds_until_up() {
    let connection = FakeTable::link_without_frame_with(FULL_BLOCK);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));

    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    connection.game.frame();
    let held = connection.game.injected();
    assert_eq!(connection.link.send_key(KeyCode::W, false), Ok(()));
    connection.game.frame();

    assert_eq!(held, vec!["key 0x57 down".to_owned()]);
    assert_eq!(connection.game.injected(), vec!["key 0x57 down".to_owned(), "key 0x57 up".to_owned()]);
}

#[test]
fn pointer_and_move_go_through_the_lease() {
    let connection = FakeTable::link_without_frame_with(FULL_BLOCK);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));

    let results = [
        connection.link.send_pointer(PointerOp::Button { button: MouseButton::Left, down: true }),
        connection.link.send_pointer(PointerOp::Wheel { notches: 1.0 }),
        connection.link.send_pointer(PointerOp::Turn(Look { yaw: 90.0, pitch: 0.0 })),
        connection.link.send_move(MoveIntent { forward: 1.0, jump: true, ..MoveIntent::default() }),
    ];
    connection.game.frame();
    let first = connection.game.injected();
    let up = connection.link.send_pointer(PointerOp::Button { button: MouseButton::Left, down: false });
    let clear = connection.link.send_move(MoveIntent::default());
    connection.game.frame();

    assert_eq!(results, [Ok(()), Ok(()), Ok(()), Ok(())]);
    assert_eq!((up, clear), (Ok(()), Ok(())));
    assert_eq!(
        first,
        vec![
            "button left down".to_owned(),
            "wheel 1".to_owned(),
            "turn 90 0".to_owned(),
            "move forward=1 jump".to_owned()
        ]
    );
    assert_eq!(connection.game.injected()[4..], ["button left up".to_owned(), "move none".to_owned()]);
}

#[test]
fn injection_without_the_lease_is_refused_but_a_clear_is_not() {
    let connection = FakeTable::link_without_frame_with(FULL_BLOCK);

    let key = connection.link.send_key(KeyCode::W, true);
    let clear = connection.link.send_move(MoveIntent::default());

    assert_eq!(key, Err(LinkError::Api(ApiError::Refused)));
    assert_eq!(clear, Ok(()));
}
