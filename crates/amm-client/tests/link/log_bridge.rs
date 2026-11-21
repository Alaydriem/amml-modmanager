use amm_client::link::LogBridge;
use amml_api::testing::FakeGame;

#[test]
fn log_bridge_writes_curia_lines_to_the_mod_log() {
    let game = FakeGame::new();
    LogBridge::install(game.ctx().log());

    curia::info!("chunk skipped", { chunk: 12, dim: "overworld" });
    curia::error!("no fields");

    assert_eq!(game.logs(), vec!["info chunk skipped chunk=12 dim=overworld".to_owned(), "error no fields".to_owned()]);
}

#[test]
fn a_second_install_keeps_the_first_bridge() {
    let game = FakeGame::new();
    LogBridge::install(game.ctx().log());
    LogBridge::install(game.ctx().log());

    curia::warn!("once");

    assert_eq!(game.logs(), vec!["warn once".to_owned()]);
}
