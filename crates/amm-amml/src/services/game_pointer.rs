/// Converts the real cursor to the game's mouse position while a mod shows the game picture in a viewport: the
/// game hit-tests with the position in its mouse feed, so a cursor inside the viewport is moved to the matching
/// place of the full window. The host decides where; this converts the real cursor to back-buffer pixels, and the
/// host's point is used as it is, in the pixels of the game's mouse feed.
pub struct GamePointer;

impl GamePointer {
    /// `cursor` and `client` are in client pixels, `buffer` is the back-buffer size. `host` takes the cursor in
    /// back-buffer pixels and the client size, and maps them to its game point in client pixels, or `None` outside
    /// the viewport.
    pub fn map(
        cursor: (i32, i32),
        client: (i32, i32),
        buffer: (u32, u32),
        host: impl FnOnce(f32, f32, u32, u32) -> Option<(f32, f32)>,
    ) -> Option<(i16, i16)> {
        if client.0 <= 0 || client.1 <= 0 || buffer.0 == 0 || buffer.1 == 0 {
            return None;
        }
        let to_buffer_x = buffer.0 as f32 / client.0 as f32;
        let to_buffer_y = buffer.1 as f32 / client.1 as f32;
        let (game_x, game_y) =
            host(cursor.0 as f32 * to_buffer_x, cursor.1 as f32 * to_buffer_y, client.0 as u32, client.1 as u32)?;
        Some((game_x.round() as i16, game_y.round() as i16))
    }
}
