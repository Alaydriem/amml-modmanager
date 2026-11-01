use time::OffsetDateTime;

/// The stamp shared by every log file of one session, in Curia's archive stamp layout
/// `YYYY-MM-DD_HH-MM-SS-mmm`. Fixed width, so stamps order as text the way they order as instants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionStamp(String);

impl SessionStamp {
    const SHAPE: &'static str = "DDDD-DD-DD_DD-DD-DD-DDD";
    const EXTENSION: &'static str = ".log";

    pub fn new(now: OffsetDateTime) -> SessionStamp {
        Self(format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}-{:03}",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.millisecond(),
        ))
    }

    /// Reads a stamp back, or `None` when the text does not match the layout exactly.
    pub fn parse(text: &str) -> Option<SessionStamp> {
        let matches = text.len() == Self::SHAPE.len()
            && text.bytes().zip(Self::SHAPE.bytes()).all(|(actual, expected)| match expected {
                b'D' => actual.is_ascii_digit(),
                separator => actual == separator,
            });
        matches.then(|| Self(text.to_string()))
    }

    /// The stamp of a session file name `<stamp>.log`, or `None` for any other name.
    pub fn from_file_name(name: &str) -> Option<SessionStamp> {
        Self::parse(name.strip_suffix(Self::EXTENSION)?)
    }

    /// The session of a log file name: `<stamp>.log` for the active file, or Curia's archive name
    /// `<stamp>_<archive stamp>[-<n>].log`. `None` for any other name.
    pub fn from_log_file_name(name: &str) -> Option<SessionStamp> {
        let body = name.strip_suffix(Self::EXTENSION)?;
        let (session, tail) = body.split_at_checked(Self::SHAPE.len())?;
        let session = Self::parse(session)?;
        if tail.is_empty() {
            return Some(session);
        }
        let archive = tail.strip_prefix('_')?;
        let (stamp, sequence) = archive.split_at_checked(Self::SHAPE.len())?;
        Self::parse(stamp)?;
        let valid_sequence = sequence.is_empty()
            || sequence.strip_prefix('-').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
        valid_sequence.then_some(session)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn file_name(&self) -> String {
        format!("{}{}", self.0, Self::EXTENSION)
    }
}
