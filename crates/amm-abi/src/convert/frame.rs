use amm_core::services::abi::AbiPrefix;
use amm_core::structs::frame::{CursorShape, FrameInputData, FrameReplyData, UiEvent, UiEventKind};
use amm_core::traits::AbiSized;

use crate::types::{AmmCursorShape, AmmFrameInput, AmmFrameOutput, AmmStatus, AmmUiEvent, AmmUiEventKind};

impl From<UiEventKind> for AmmUiEventKind {
    fn from(value: UiEventKind) -> Self {
        match value {
            UiEventKind::PointerMove => AmmUiEventKind::PointerMove,
            UiEventKind::PointerButton => AmmUiEventKind::PointerButton,
            UiEventKind::Wheel => AmmUiEventKind::Wheel,
            UiEventKind::Key => AmmUiEventKind::Key,
            UiEventKind::Char => AmmUiEventKind::Char,
            UiEventKind::FocusLost => AmmUiEventKind::FocusLost,
        }
    }
}

impl From<AmmUiEventKind> for UiEventKind {
    fn from(value: AmmUiEventKind) -> Self {
        match value {
            AmmUiEventKind::PointerMove => UiEventKind::PointerMove,
            AmmUiEventKind::PointerButton => UiEventKind::PointerButton,
            AmmUiEventKind::Wheel => UiEventKind::Wheel,
            AmmUiEventKind::Key => UiEventKind::Key,
            AmmUiEventKind::Char => UiEventKind::Char,
            AmmUiEventKind::FocusLost => UiEventKind::FocusLost,
        }
    }
}

impl From<UiEvent> for AmmUiEvent {
    fn from(value: UiEvent) -> Self {
        Self {
            kind: value.kind.into(),
            x: value.x,
            y: value.y,
            key: value.key,
            modifiers: value.modifiers,
            down: value.down,
            ch: value.ch,
        }
    }
}

impl From<AmmUiEvent> for UiEvent {
    fn from(value: AmmUiEvent) -> Self {
        Self {
            kind: value.kind.into(),
            x: value.x,
            y: value.y,
            key: value.key,
            modifiers: value.modifiers,
            down: value.down,
            ch: value.ch,
        }
    }
}

impl From<CursorShape> for AmmCursorShape {
    fn from(value: CursorShape) -> Self {
        match value {
            CursorShape::Default => AmmCursorShape::Default,
            CursorShape::Arrow => AmmCursorShape::Arrow,
            CursorShape::Hand => AmmCursorShape::Hand,
            CursorShape::Text => AmmCursorShape::Text,
        }
    }
}

impl From<AmmCursorShape> for CursorShape {
    fn from(value: AmmCursorShape) -> Self {
        match value {
            AmmCursorShape::Default => CursorShape::Default,
            AmmCursorShape::Arrow => CursorShape::Arrow,
            AmmCursorShape::Hand => CursorShape::Hand,
            AmmCursorShape::Text => CursorShape::Text,
        }
    }
}

impl From<&FrameInputData<'_>> for AmmFrameInput {
    fn from(value: &FrameInputData<'_>) -> Self {
        Self {
            width: value.width,
            height: value.height,
            pixels_per_point: value.pixels_per_point,
            time_secs: value.time_secs,
            // UiEvent is #[repr(C)] with the same field order and types as AmmUiEvent, which
            // tests/types/layout.rs checks.
            events: value.events.as_ptr().cast::<AmmUiEvent>(),
            event_count: value.events.len(),
            allowed_layers: value.allowed_layers,
            input_flags: value.input_flags,
            ..Self::blank()
        }
    }
}

impl TryFrom<u8> for AmmCursorShape {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmCursorShape::Default),
            1 => Ok(AmmCursorShape::Arrow),
            2 => Ok(AmmCursorShape::Hand),
            3 => Ok(AmmCursorShape::Text),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}

impl TryFrom<&AmmFrameOutput> for FrameReplyData {
    type Error = AmmStatus;

    fn try_from(value: &AmmFrameOutput) -> Result<Self, Self::Error> {
        AbiPrefix::len(value.size, AmmFrameOutput::BASE_SIZE, size_of::<AmmFrameOutput>())
            .ok_or(AmmStatus::InvalidArgument)?;
        let flag = |raw: u8| match raw {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(AmmStatus::InvalidArgument),
        };
        Ok(Self {
            cursor: AmmCursorShape::try_from(value.cursor)?.into(),
            wants_pointer: flag(value.wants_pointer)?,
            wants_keyboard: flag(value.wants_keyboard)?,
            layer_rect: value.layer_rect,
        })
    }
}
