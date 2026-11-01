use serde::Deserialize;

/// The manager's bounds. Per mod and per frame, summed over the Hud and Overlay layers of the mod:
/// `vertices_per_mod`, `indices_per_mod` and `draw_cmds_per_mod` cap the draw (a frame over any of them is
/// drawn without its mesh, its textures still processed);
/// `upload_bytes_per_frame` caps the texture bytes one mod uploads in one frame (later deltas are refused);
/// `freed_ids_per_frame` caps the texture ids one mod frees in one frame (later ids are dropped);
/// `texture_deltas_per_frame` caps the texture deltas one mod sends in one frame and `textures_per_mod` the
/// textures one mod keeps (later deltas are refused and counted). `texture_bytes_per_mod` and `textures_per_mod` are
/// the texture budget of a mod whose descriptor asks for none; `max_texture_bytes_per_mod` and
/// `max_textures_per_mod` are the most a mod gets when its descriptor asks for more (`TextureGranter`).
///
/// A hand-edited value outside its bound is clamped when the manager config is read (`clamped`): each field has
/// a fixed maximum, and the fields where 0 breaks the manager have a minimum (`max_mods`, `toasts`,
/// `frame_failure_limit`, `texture_deltas_per_frame`, `textures_per_mod` and `max_textures_per_mod` at least 1,
/// `io_queue` at least 1, `event_queue` at least `MIN_EVENT_QUEUE`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Budgets {
    pub max_mods: usize,
    pub event_queue: usize,
    pub toasts: usize,
    pub toast_rate_ms: u64,
    pub texture_bytes_per_mod: u64,
    pub vertices_per_mod: u32,
    pub indices_per_mod: u32,
    pub draw_cmds_per_mod: u32,
    pub upload_bytes_per_frame: u64,
    pub freed_ids_per_frame: u32,
    pub io_queue: usize,
    pub write_merge_ms: u64,
    pub frame_failure_limit: u32,
    pub max_keybinds: usize,
    pub max_fields: usize,
    pub texture_deltas_per_frame: usize,
    pub textures_per_mod: usize,
    pub max_texture_bytes_per_mod: u64,
    pub max_textures_per_mod: usize,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            max_mods: 32,
            event_queue: 256,
            toasts: 8,
            toast_rate_ms: 2000,
            texture_bytes_per_mod: 64 * 1024 * 1024,
            vertices_per_mod: 262_144,
            indices_per_mod: 786_432,
            draw_cmds_per_mod: 16_384,
            upload_bytes_per_frame: 16 * 1024 * 1024,
            freed_ids_per_frame: 4_096,
            io_queue: 32,
            write_merge_ms: 500,
            frame_failure_limit: 30,
            max_keybinds: 128,
            max_fields: 256,
            texture_deltas_per_frame: 256,
            textures_per_mod: 1_024,
            max_texture_bytes_per_mod: 1024 * 1024 * 1024,
            max_textures_per_mod: 4_096,
        }
    }
}

impl Budgets {
    pub const MAX_MODS: usize = 32;
    pub const MIN_EVENT_QUEUE: usize = 16;
    pub const MAX_EVENT_QUEUE: usize = 4_096;
    pub const MAX_TOASTS: usize = 64;
    pub const MAX_TOAST_RATE_MS: u64 = 60_000;
    pub const MAX_TEXTURE_BYTES_PER_MOD: u64 = 1024 * 1024 * 1024;
    pub const MAX_VERTICES_PER_MOD: u32 = 1_048_576;
    pub const MAX_INDICES_PER_MOD: u32 = 3_145_728;
    pub const MAX_DRAW_CMDS_PER_MOD: u32 = 65_536;
    pub const MAX_UPLOAD_BYTES_PER_FRAME: u64 = 64 * 1024 * 1024;
    pub const MAX_FREED_IDS_PER_FRAME: u32 = 16_384;
    pub const MAX_IO_QUEUE: usize = 256;
    pub const MAX_WRITE_MERGE_MS: u64 = 10_000;
    pub const MAX_FRAME_FAILURE_LIMIT: u32 = 1_000;
    pub const MAX_KEYBINDS: usize = 512;
    pub const MAX_FIELDS: usize = 1_024;
    pub const MAX_TEXTURE_DELTAS_PER_FRAME: usize = 4_096;
    pub const MAX_TEXTURES_PER_MOD: usize = 4_096;
    pub const HARD_MAX_TEXTURE_BYTES_PER_MOD: u64 = 2048 * 1024 * 1024;
    /// The overlay texture registry's mod space: its 8192 textures less the 64 kept for the manager. The registry is
    /// shared by all mods, so a grant is a limit, not reserved space.
    pub const HARD_MAX_TEXTURES_PER_MOD: usize = 8_192 - 64;

    /// These budgets with each field inside its bound, and the names of the fields that were changed, in field
    /// order.
    pub fn clamped(&self) -> (Budgets, Vec<&'static str>) {
        let mut names = Vec::new();
        let names_ref = &mut names;
        let clamped = Budgets {
            max_mods: Self::bound(self.max_mods, 1, Self::MAX_MODS, "max_mods", names_ref),
            event_queue: Self::bound(
                self.event_queue,
                Self::MIN_EVENT_QUEUE,
                Self::MAX_EVENT_QUEUE,
                "event_queue",
                names_ref,
            ),
            toasts: Self::bound(self.toasts, 1, Self::MAX_TOASTS, "toasts", names_ref),
            toast_rate_ms: Self::bound(self.toast_rate_ms, 0, Self::MAX_TOAST_RATE_MS, "toast_rate_ms", names_ref),
            texture_bytes_per_mod: Self::bound(
                self.texture_bytes_per_mod,
                0,
                Self::MAX_TEXTURE_BYTES_PER_MOD,
                "texture_bytes_per_mod",
                names_ref,
            ),
            vertices_per_mod: Self::bound(
                self.vertices_per_mod,
                0,
                Self::MAX_VERTICES_PER_MOD,
                "vertices_per_mod",
                names_ref,
            ),
            indices_per_mod: Self::bound(
                self.indices_per_mod,
                0,
                Self::MAX_INDICES_PER_MOD,
                "indices_per_mod",
                names_ref,
            ),
            draw_cmds_per_mod: Self::bound(
                self.draw_cmds_per_mod,
                0,
                Self::MAX_DRAW_CMDS_PER_MOD,
                "draw_cmds_per_mod",
                names_ref,
            ),
            upload_bytes_per_frame: Self::bound(
                self.upload_bytes_per_frame,
                0,
                Self::MAX_UPLOAD_BYTES_PER_FRAME,
                "upload_bytes_per_frame",
                names_ref,
            ),
            freed_ids_per_frame: Self::bound(
                self.freed_ids_per_frame,
                0,
                Self::MAX_FREED_IDS_PER_FRAME,
                "freed_ids_per_frame",
                names_ref,
            ),
            io_queue: Self::bound(self.io_queue, 1, Self::MAX_IO_QUEUE, "io_queue", names_ref),
            write_merge_ms: Self::bound(self.write_merge_ms, 0, Self::MAX_WRITE_MERGE_MS, "write_merge_ms", names_ref),
            frame_failure_limit: Self::bound(
                self.frame_failure_limit,
                1,
                Self::MAX_FRAME_FAILURE_LIMIT,
                "frame_failure_limit",
                names_ref,
            ),
            max_keybinds: Self::bound(self.max_keybinds, 0, Self::MAX_KEYBINDS, "max_keybinds", names_ref),
            max_fields: Self::bound(self.max_fields, 0, Self::MAX_FIELDS, "max_fields", names_ref),
            texture_deltas_per_frame: Self::bound(
                self.texture_deltas_per_frame,
                1,
                Self::MAX_TEXTURE_DELTAS_PER_FRAME,
                "texture_deltas_per_frame",
                names_ref,
            ),
            textures_per_mod: Self::bound(
                self.textures_per_mod,
                1,
                Self::MAX_TEXTURES_PER_MOD,
                "textures_per_mod",
                names_ref,
            ),
            max_texture_bytes_per_mod: Self::bound(
                self.max_texture_bytes_per_mod,
                0,
                Self::HARD_MAX_TEXTURE_BYTES_PER_MOD,
                "max_texture_bytes_per_mod",
                names_ref,
            ),
            max_textures_per_mod: Self::bound(
                self.max_textures_per_mod,
                1,
                Self::HARD_MAX_TEXTURES_PER_MOD,
                "max_textures_per_mod",
                names_ref,
            ),
        };
        (clamped, names)
    }

    fn bound<T: Ord + Copy>(value: T, min: T, max: T, name: &'static str, names: &mut Vec<&'static str>) -> T {
        let bounded = value.clamp(min, max);
        if bounded != value {
            names.push(name);
        }
        bounded
    }
}
