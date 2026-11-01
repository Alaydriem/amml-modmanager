use std::sync::Arc;

use curia::{LineFormatter, LogEvent};

/// The one line layout of every session file and console line:
/// `<rfc3339 time> <LEVEL> <target> <message> <fields json> [file:line]`.
pub struct LineFormat;

impl LineFormat {
    pub fn formatter() -> LineFormatter {
        Arc::new(|event: &LogEvent| Self::render(event))
    }

    pub fn render(event: &LogEvent) -> String {
        let mut line = format!(
            "{} {:<5} {} {}",
            event.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event.level.as_str().to_ascii_uppercase(),
            event.target,
            event.message,
        );
        if !event.fields.is_empty() {
            line.push(' ');
            line.push_str(&serde_json::to_string(event.fields.as_map()).unwrap_or_default());
        }
        if let (Some(file), Some(number)) = (&event.file, event.line) {
            line.push_str(&format!(" [{file}:{number}]"));
        }
        line
    }
}
