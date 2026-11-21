use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use amm_client::link::{LinkState, ManagerLink, ManagerTables};
use amm_client::test_support::FakeTable;
use amm_core::errors::LinkError;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amml_api::amml_abi::{AmmlStr, ProviderGone, ProviderReady, ScreenshotTaken};
use amml_api::testing::FakeGame;
use amml_api::{HookOrder, Provided, ShotId};

use crate::support::{ConsumerMod, DropFlagHandler, ManagerMod};

type SharedLink = Rc<RefCell<Option<ManagerLink>>>;

fn registers() -> usize {
    FakeTable::calls().iter().filter(|call| **call == "register").count()
}

fn ready(name: &'static str) -> ProviderReady {
    let major = amm_abi::types::AmmCoreTable::MAJOR;
    ProviderReady { header: FakeGame::header::<ProviderReady>(), name: AmmlStr::from_str(name), major, minor: 0 }
}

fn gone(name: &'static str) -> ProviderGone {
    let major = amm_abi::types::AmmCoreTable::MAJOR;
    ProviderGone { header: FakeGame::header::<ProviderGone>(), name: AmmlStr::from_str(name), major }
}

// A started link on a shared fake loader with the tables provided and no `ProviderReady` raised yet. The link sits
// in an `Rc` so a table hook can call it.
fn shared_waiting() -> (Rc<FakeGame>, SharedLink) {
    let game = Rc::new(FakeTable::game());
    let ctx = FakeTable::ctx_of(&game, FakeTable::DESCRIPTOR);
    let link = match ManagerLink::start_without_frame(&ctx, FakeTable::DESCRIPTOR) {
        Ok(link) => link,
        Err(error) => panic!("start failed: {error}"),
    };
    if let Err(error) = FakeTable::provide(&ctx) {
        panic!("provide failed: {error}");
    }
    (game, Rc::new(RefCell::new(Some(link))))
}

// Drops the hook (it holds the fake loader) and then the link before the loader.
fn finish(game: Rc<FakeGame>, link: SharedLink) {
    FakeTable::with(|state| state.on_call = None);
    drop(link.borrow_mut().take());
    drop(game);
}

#[test]
fn a_link_started_after_the_tables_are_provided_connects_at_enable() {
    let game = FakeTable::game();

    let manager = game.enable::<ManagerMod>();
    let consumer = game.enable::<ConsumerMod>();

    assert_eq!((manager, consumer), (Ok(()), Ok(())));
    let state = ConsumerMod::link().as_ref().map(ManagerLink::state);
    assert_eq!(state, Some(LinkState::Connected), "connected with no ProviderReady raised");
    assert_eq!(registers(), 1);
    assert_eq!(FakeTable::with(|state| state.config_dir.clone()), Some(FakeTable::CONFIG_DIR.to_owned()));
    game.disable::<ConsumerMod>();
}

#[test]
fn a_link_enabled_again_connects_without_a_new_provider_ready() {
    let game = FakeTable::game();
    assert_eq!(game.enable::<ManagerMod>(), Ok(()));
    assert_eq!(game.enable::<ConsumerMod>(), Ok(()));

    game.disable::<ConsumerMod>();
    let unregistered = FakeTable::with(|state| state.unregistered.len());
    assert!(ConsumerMod::link().is_none());
    assert_eq!(game.enable::<ManagerMod>(), Ok(()));
    assert_eq!(game.enable::<ConsumerMod>(), Ok(()));

    assert_eq!(unregistered, 1, "the disable unregistered the first link");
    let state = ConsumerMod::link().as_ref().map(ManagerLink::state);
    assert_eq!(state, Some(LinkState::Connected));
    assert_eq!(registers(), 2);
    game.disable::<ConsumerMod>();
}

#[test]
fn an_event_raised_from_inside_register_sees_no_half_made_connect() {
    let (game, link) = shared_waiting();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (hook_game, hook_link, hook_seen) = (Rc::clone(&game), Rc::clone(&link), Rc::clone(&seen));
    FakeTable::on_call(move |name| {
        if name != "register" {
            return;
        }
        // `ModsLoaded` reaches the link's other listener; a nested `ProviderReady` may be held back as the same
        // listener runs already
        let _ = hook_game.raise(&mut ready(ManagerTables::NAMES[1]));
        FakeTable::mods_loaded(&hook_game);
        let link = hook_link.borrow();
        let Some(link) = link.as_ref() else { return };
        hook_seen.borrow_mut().push((link.state(), link.set_setting("a", &hcl::Value::from(1))));
    });

    FakeTable::ready(&game);

    assert_eq!(*seen.borrow(), vec![(LinkState::Waiting, Err(LinkError::NotConnected))]);
    assert_eq!(link.borrow().as_ref().map(ManagerLink::state), Some(LinkState::Connected));
    assert_eq!(registers(), 1, "the ProviderReady inside register started no second connect");
    finish(game, link);
}

#[test]
fn provider_gone_raised_from_inside_register_leaves_the_link_gone() {
    let (game, link) = shared_waiting();
    let hook_game = Rc::clone(&game);
    FakeTable::on_call(move |name| {
        if name == "register" {
            let _ = hook_game.raise(&mut gone(ManagerTables::NAMES[0]));
        }
    });

    FakeTable::ready(&game);

    let handle = FakeTable::with(|state| state.handle);
    assert_eq!(link.borrow().as_ref().map(ManagerLink::state), Some(LinkState::Gone));
    assert_eq!(
        FakeTable::with(|state| state.unregistered.clone()),
        vec![handle],
        "the late registration is given back"
    );
    let events = link.borrow_mut().as_mut().map(|link| link.events().collect::<Vec<_>>());
    assert_eq!(events, Some(vec![ModEvent::ManagerGone]));
    finish(game, link);
}

#[test]
fn an_event_raised_from_inside_unregister_at_provider_gone_sees_the_link_gone() {
    let (game, link) = shared_waiting();
    FakeTable::ready(&game);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (hook_link, hook_seen) = (Rc::clone(&link), Rc::clone(&seen));
    FakeTable::on_call(move |name| {
        if name != "unregister" {
            return;
        }
        let link = hook_link.borrow();
        let Some(link) = link.as_ref() else { return };
        hook_seen.borrow_mut().push((link.state(), link.panel_open()));
    });

    FakeTable::gone(&game);

    assert_eq!(*seen.borrow(), vec![(LinkState::Gone, false)]);
    finish(game, link);
}

#[test]
fn an_event_raised_from_inside_poll_events_keeps_its_place() {
    let (game, link) = shared_waiting();
    FakeTable::ready(&game);
    FakeTable::push_event(ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed });
    let raised = Rc::new(Cell::new(false));
    let (hook_game, hook_raised) = (Rc::clone(&game), Rc::clone(&raised));
    FakeTable::on_call(move |name| {
        if name == "poll_events" && !hook_raised.replace(true) {
            let header = FakeGame::header::<ScreenshotTaken>();
            let _ = hook_game.raise(&mut ScreenshotTaken { header, id: 7, ok: 1, _reserved: [0; 7] });
        }
    });

    let events = link.borrow_mut().as_mut().map(|link| link.events().collect::<Vec<_>>());

    let Some(events) = events else { panic!("no link") };
    assert!(raised.get());
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(events.contains(&ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed }));
    assert!(events.contains(&ModEvent::ScreenshotTaken { id: ShotId(7), ok: true }));
    finish(game, link);
}

#[test]
fn a_listener_that_calls_the_link_from_inside_provider_ready_finishes() {
    let game = FakeTable::game();
    let ctx = FakeTable::ctx_of(&game, FakeTable::DESCRIPTOR);
    let link = Arc::new(Mutex::new(ManagerLink::start_without_frame(&ctx, FakeTable::DESCRIPTOR).ok()));
    let listener_link = Arc::clone(&link);
    let answers = Arc::new(Mutex::new(Vec::new()));
    let listener_answers = Arc::clone(&answers);
    let listener = ctx.events().listen::<ProviderReady>(HookOrder::LAST, move |_, _| {
        let link = listener_link.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(link) = link.as_ref() {
            let answer = link.set_setting("card.scale", &hcl::Value::from(2));
            listener_answers.lock().unwrap_or_else(PoisonError::into_inner).push((link.state(), answer));
        }
    });
    let Ok(listener) = listener else { panic!("listen refused") };
    if let Err(error) = FakeTable::provide(&ctx) {
        panic!("provide failed: {error}");
    }

    let _ = game.raise(&mut ready(ManagerTables::NAMES[0]));

    let answers = answers.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(answers, vec![(LinkState::Connected, Ok(()))]);
    assert_eq!(FakeTable::with(|state| state.settings.len()), 1);
    drop(listener);
    drop(link.lock().unwrap_or_else(PoisonError::into_inner).take());
}

#[test]
fn a_link_dropped_during_its_connect_keeps_the_frame_state_until_the_registration_is_given_back() {
    let game = Rc::new(FakeTable::game());
    let ctx = FakeTable::ctx_of(&game, FakeTable::DESCRIPTOR);
    let dropped = Arc::new(AtomicBool::new(false));
    let started = ManagerLink::start(&ctx, FakeTable::DESCRIPTOR, Some(DropFlagHandler::new(Arc::clone(&dropped))));
    let Ok(started) = started else { panic!("start failed") };
    if let Err(error) = FakeTable::provide(&ctx) {
        panic!("provide failed: {error}");
    }
    let link: SharedLink = Rc::new(RefCell::new(Some(started)));
    let freed_at_callbacks = Rc::new(Cell::new(None));
    let (hook_link, hook_dropped, hook_freed) =
        (Rc::clone(&link), Arc::clone(&dropped), Rc::clone(&freed_at_callbacks));
    FakeTable::on_call(move |name| match name {
        "register" => drop(hook_link.borrow_mut().take()),
        "set_callbacks" => hook_freed.set(Some(hook_dropped.load(Ordering::SeqCst))),
        _ => {}
    });

    FakeTable::ready(&game);

    let handle = FakeTable::with(|state| state.handle);
    assert_eq!(freed_at_callbacks.get(), Some(false), "the state lives while the manager gets the callbacks");
    assert_eq!(FakeTable::with(|state| state.unregistered.clone()), vec![handle]);
    assert!(dropped.load(Ordering::SeqCst), "freed after the Ok unregister");
    finish(game, link);
}
