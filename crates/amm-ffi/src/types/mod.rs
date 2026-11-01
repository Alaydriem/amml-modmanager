pub mod host_console_callback;
pub mod host_fake_key;
pub mod host_frame;
pub mod host_frame_input;
pub mod host_layer;
pub mod host_pointer_kind;
pub mod host_route;

pub use host_console_callback::AmmHostConsoleCallback;
pub use host_fake_key::AmmHostFakeKey;
pub use host_frame::{
    AMM_HOST_CAPTURE_MAX_FRAMES, AMM_HOST_GAME_PICTURE_TEXTURE, AMM_HOST_MAX_CAPTURES, AMM_HOST_MAX_STREAM_FRAMES,
    AmmHostFrame,
};
pub use host_frame_input::AmmHostFrameInput;
pub use host_layer::AmmHostLayer;
pub use host_pointer_kind::AmmHostPointerKind;
pub use host_route::AmmHostRoute;
