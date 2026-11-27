use amm_client::link::ManagerTables;
use amm_client::test_support::FakeTable;

#[test]
fn the_fake_provides_the_five_tables_under_their_names() {
    let game = FakeTable::game();
    let ctx = game.ctx();
    let before = ManagerTables::require(&ctx.registry()).err();

    let provided = FakeTable::provide(&ctx);

    assert_eq!(before, Some("alaydriem.modmanager.core"));
    assert_eq!(provided, Ok(()));
    let Ok(tables) = ManagerTables::require(&ctx.registry()) else { panic!("the five tables are not required") };
    assert!(tables.core.register.is_some() && tables.leases.held_by_other.is_some());
    assert_eq!(
        ManagerTables::NAMES,
        [
            "alaydriem.modmanager.core",
            "alaydriem.modmanager.settings",
            "alaydriem.modmanager.keybinds",
            "alaydriem.modmanager.layers",
            "alaydriem.modmanager.leases",
        ]
    );
}
