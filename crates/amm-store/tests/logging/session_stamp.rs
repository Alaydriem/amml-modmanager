use amm_store::logging::SessionStamp;
use time::macros::datetime;

#[test]
fn file_name_uses_the_fixed_width_curia_stamp() {
    let stamp = SessionStamp::new(datetime!(2026-01-02 03:04:05.006 UTC));

    assert_eq!(stamp.file_name(), "2026-01-02_03-04-05-006.log");
}

#[test]
fn parse_accepts_only_the_exact_shape() {
    assert!(SessionStamp::parse("2026-08-20_14-30-00-123").is_some());
    for text in ["2026-8-20_14-30-00-123", "2026-08-20_14-30-00-12", "2026-08-20-14-30-00-123", "", "latest"] {
        assert!(SessionStamp::parse(text).is_none(), "accepted {text}");
    }
}

#[test]
fn stamps_order_as_instants() {
    let earlier = SessionStamp::new(datetime!(2026-08-20 23:59:59.999 UTC));
    let later = SessionStamp::new(datetime!(2026-08-21 00:00:00.000 UTC));

    assert!(earlier < later);
}
