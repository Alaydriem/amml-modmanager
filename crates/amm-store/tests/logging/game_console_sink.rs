use amm_store::logging::GameConsoleSink;
use curia::{Fields, Level, LogEvent, Sink};

fn event(level: Level) -> LogEvent {
    LogEvent {
        level,
        target: "test".into(),
        message: "hello".into(),
        fields: Fields::new(),
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    }
}

#[test]
fn console_queue_past_capacity_drops_and_counts() {
    let (tx, rx) = GameConsoleSink::channel();
    let sink = GameConsoleSink::new(tx);

    for _ in 0..(GameConsoleSink::CAPACITY + 3) {
        sink.emit(&event(Level::Warn));
    }

    assert_eq!(rx.len(), GameConsoleSink::CAPACITY);
    assert_eq!(sink.dropped(), 3);
}

#[test]
fn console_admits_warn_and_error_only() {
    let (tx, rx) = GameConsoleSink::channel();
    let sink = GameConsoleSink::new(tx);

    assert_eq!(sink.level(), Level::Warn);
    sink.emit(&event(Level::Error));
    sink.emit(&event(Level::Info));

    let (level, line) = rx.try_recv().unwrap();
    assert_eq!(level, Level::Error);
    assert!(line.contains("hello"));
    assert!(rx.try_recv().is_err());
}
