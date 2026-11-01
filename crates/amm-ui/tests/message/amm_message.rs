use std::path::Path;

use amm_ui::message::AmmMessage;
use amm_ui::text::AmmPacks;
use amml_i18n::message::Message;
use amml_i18n::service::{LocaleSetting, LocalizationService};
use amml_i18n_tool::catalog::CatalogCheck;

#[test]
fn message_catalog() {
    let problems = CatalogCheck::problems::<AmmMessage>();
    assert!(problems.is_empty(), "{}", problems.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
}

#[test]
fn pot_up_to_date() {
    let i18n = Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let current = CatalogCheck::pot_is_current::<AmmMessage>(&i18n).unwrap_or_else(|error| panic!("{error}"));
    assert!(current, "amm-ui.pot is out of date: run `mise run i18n:extract`");
}

#[test]
fn no_glyph_separator_in_a_message() {
    for message in AmmMessage::ALL {
        for glyph in ['\u{b7}', '\u{203a}'] {
            assert!(!message.msgid().contains(glyph), "{message:?} holds a separator glyph");
        }
    }
}

#[test]
fn pseudo_pack_covers_every_message() {
    let localizer = LocalizationService::new(vec![(AmmMessage::DOMAIN, AmmPacks::SOURCE)]).load(&LocaleSetting::Pseudo);
    let table = localizer.table::<AmmMessage>();
    for message in AmmMessage::ALL {
        assert_ne!(table.text(*message), message.msgid(), "{message:?} has no pseudo text");
    }
}
