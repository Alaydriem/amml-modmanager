use amm_bench::fixtures::ClientRectProbe;

#[test]
fn the_hidden_window_reports_its_client_size() {
    let probe = ClientRectProbe::new();

    assert_eq!(probe.read(), (ClientRectProbe::WIDTH, ClientRectProbe::HEIGHT));
}
