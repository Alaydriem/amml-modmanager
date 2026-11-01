use amm_abi::types::{AmmHandle, AmmLogLevel};
use amm_client::link::ForwardSink;
use curia::{Fields, Level, LogEvent, Sink};

use crate::support::{FakeTable, LoggedCall};

#[test]
fn forward_sink_passes_level_message_and_fields_json_to_the_table() {
    let sink = ForwardSink::new(FakeTable::table(), AmmHandle { slot: 1, generation: 2 }, Level::Debug);
    let mut fields = Fields::new();
    fields.insert("chunk", 12);
    fields.insert("dim", "overworld");
    let event = LogEvent {
        level: Level::Warn,
        target: "demo::scan".to_owned(),
        message: "chunk skipped".to_owned(),
        fields,
        timestamp: chrono::Utc::now(),
        file: Some("src/scan.rs".to_owned()),
        line: Some(42),
    };

    sink.emit(&event);

    assert_eq!(
        FakeTable::with(|state| state.logs.clone()),
        vec![LoggedCall {
            level: AmmLogLevel::Warn as u8,
            target: "demo::scan".to_owned(),
            message: "chunk skipped".to_owned(),
            fields_json: "{\"chunk\":12,\"dim\":\"overworld\"}".to_owned(),
            file: "src/scan.rs".to_owned(),
            line: 42,
        }]
    );
}

#[test]
fn a_detached_forward_sink_sends_nothing() {
    let sink = ForwardSink::new(FakeTable::table(), AmmHandle { slot: 1, generation: 2 }, Level::Debug);
    let event = LogEvent {
        level: Level::Error,
        target: "demo".to_owned(),
        message: "late".to_owned(),
        fields: Fields::new(),
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    };

    sink.detach();
    sink.emit(&event);

    assert!(FakeTable::with(|state| state.logs.is_empty()));
}
