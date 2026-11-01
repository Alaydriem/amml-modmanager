use std::time::Instant;

use amm_abi::types::{AmmCursorShape, AmmDrawCmd, AmmFrameOutput, AmmMeshView, AmmStatus, AmmTextureDelta, AmmVertex};
use amm_core::config::Budgets;
use amm_core::services::telemetry::Telemetry;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::frame::{FrameInputData, FrameReplyData, LayerKind, MenuScreen};
use amm_core::structs::manager::TextureGrant;
use amm_core::structs::mesh::{DrawCmd, GamePicture, MeshBuffers, TextureDelta};
use amm_core::structs::registry::{ModIndex, ModMask};
use amm_core::structs::snapshot::{LayerEntry, RenderSnapshot};
use amm_ui::model::PanelAction;

use super::{HostRuntime, OwnerFrameUse, TextureBudget};
use crate::types::{AmmHostFrame, AmmHostFrameInput, AmmHostLayer};

/// Builds the layer list of one Present. It calls the frame callback of each drawn layer, once per layer with that
/// layer's one bit in `allowed_layers`, checks what comes back, keeps each mod inside its vertex and texture
/// budgets, runs the panel pass, and fills a reused layer array. The two layers of one mod share its owner byte, its
/// texture ids and its budgets: the per-frame budgets count the sum of its layers in one compose. Every layer points
/// only at this compositor's own copies of the mesh (vertices, indices, commands, admitted deltas and their bytes,
/// freed ids), one set of copies per layer, so a layer stays valid until the next compose even when its mod
/// unregisters and frees its own buffers. An owner that leaves is only marked; its copies are released at the start
/// of the next compose, after the overlay has finished reading the last frame. Each draw command is copied with its clip
/// rect inside the back buffer; empty and NaN clips are dropped. The copies are reused across frames, so a steady
/// frame does not allocate. Bounds: `RenderSnapshot::MAX_LAYERS` mod layers plus the manager's. The copies of one
/// owner's layers hold one frame at most and together at most `vertices_per_mod` vertices, `indices_per_mod`
/// indices, `draw_cmds_per_mod` commands, `texture_deltas_per_frame` deltas holding at most `upload_bytes_per_frame`
/// bytes, and `freed_ids_per_frame` freed ids. They keep their capacity while the owner stays, and are released at
/// the first compose after it leaves.
pub struct FrameCompositor {
    layers: Vec<AmmHostLayer>,
    // one bit per owner slot whose copies wait for release at the next compose
    released: u64,
    // what each owner used of its per-frame budgets in this compose
    used: [OwnerFrameUse; Self::OWNERS],
    // one bit per mod slot with a layer that failed in this compose, so the mod's frame counts one failure
    failed: u32,
    // one bit per mod slot with a layer that succeeded in this compose
    succeeded: u32,
    // a layer of this compose kept a draw command of the game picture
    game_picture: bool,
    vertices: Vec<Vec<AmmVertex>>,
    indices: Vec<Vec<u32>>,
    cmds: Vec<Vec<AmmDrawCmd>>,
    deltas: Vec<Vec<AmmTextureDelta>>,
    bytes: Vec<Vec<u8>>,
    freed: Vec<Vec<u64>>,
    budget: TextureBudget,
    vertex_limit: usize,
    index_limit: usize,
    cmd_limit: usize,
    upload_limit: u64,
    delta_limit: usize,
    freed_limit: usize,
    panel_mesh: MeshBuffers,
    panel_actions: Vec<PanelAction>,
    // the composed line was written for this open of the panel
    panel_logged: bool,
    // the refused line was written for this open of the panel
    refusal_logged: bool,
}

// SAFETY: the raw pointers in the layer array point only into this compositor's own copies. They are handed to
// the overlay and never read here, and the compositor is only used under the host lock.
unsafe impl Send for FrameCompositor {}

impl FrameCompositor {
    const OWNERS: usize = ModMask::CAPACITY + 1;
    const MANAGER_SLOT: usize = ModMask::CAPACITY;
    const BUFFERS: usize = RenderSnapshot::MAX_LAYERS + 1;
    const MANAGER_BUFFER: usize = RenderSnapshot::MAX_LAYERS;

    pub fn new(budgets: &Budgets) -> Self {
        Self {
            layers: Vec::with_capacity(Self::BUFFERS),
            released: 0,
            used: [OwnerFrameUse::new(); Self::OWNERS],
            failed: 0,
            succeeded: 0,
            game_picture: false,
            vertices: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            indices: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            cmds: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            deltas: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            bytes: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            freed: (0..Self::BUFFERS).map(|_| Vec::new()).collect(),
            budget: TextureBudget::new(budgets.texture_bytes_per_mod, budgets.textures_per_mod),
            delta_limit: budgets.texture_deltas_per_frame,
            vertex_limit: usize::try_from(budgets.vertices_per_mod).unwrap_or(usize::MAX),
            index_limit: usize::try_from(budgets.indices_per_mod).unwrap_or(usize::MAX),
            cmd_limit: usize::try_from(budgets.draw_cmds_per_mod).unwrap_or(usize::MAX),
            upload_limit: budgets.upload_bytes_per_frame,
            freed_limit: usize::try_from(budgets.freed_ids_per_frame).unwrap_or(usize::MAX),
            panel_mesh: MeshBuffers::new(),
            panel_actions: Vec::new(),
            panel_logged: false,
            refusal_logged: false,
        }
    }

    /// The layers of one Present, valid until the next call. With no layer, no toast, the panel closed and the
    /// menu button hidden it returns at once with no layers.
    pub fn compose(&mut self, rt: &mut HostRuntime, input: &AmmHostFrameInput) -> AmmHostFrame {
        self.layers.clear();
        self.used = [OwnerFrameUse::new(); Self::OWNERS];
        self.failed = 0;
        self.succeeded = 0;
        self.game_picture = false;
        self.release_marked(rt.manager().telemetry());
        rt.manager_mut().set_menu_screen(MenuScreen::from_code(input.menu_screen));
        let snapshot = rt.manager().snapshot();
        let toast = rt.toast_visible(input.now_ms);
        // from the replies of the last compose, so a covering layer that is gone no longer hides the row
        rt.manager_mut().update_overlay_covers((input.width, input.height));
        let button = rt.manager().menu_button_visible();
        if !snapshot.panel_open {
            self.panel_logged = false;
            self.refusal_logged = false;
        }
        if snapshot.layers.is_empty() && !snapshot.panel_open && !toast && !button {
            return self.finish(rt, AmmCursorShape::Default);
        }
        let telemetry = rt.manager().shared_telemetry();
        let pointer_layer = rt.manager().pointer_layer();
        let mut cursor = AmmCursorShape::Default;
        let mut owner_cursor = None;
        for entry in snapshot.layers.iter() {
            let Some((shape, under)) = self.client_layer(rt, entry, input, &telemetry) else {
                continue;
            };
            if pointer_layer == Some(*entry) {
                owner_cursor = Some(shape);
            } else if under {
                cursor = shape;
            }
        }
        self.end_runs(rt);
        rt.manager_mut().update_overlay_covers((input.width, input.height));
        let button = rt.manager().menu_button_visible();
        rt.clear_ui_events();
        let cursor = owner_cursor.unwrap_or(cursor);
        if snapshot.panel_open || toast || button {
            self.panel_layer(rt, input, &telemetry);
        }
        self.finish(rt, if snapshot.panel_open { AmmCursorShape::Default } else { cursor })
    }

    /// Gives a mod that registered its texture budget.
    pub fn grant_textures(&mut self, owner: u8, grant: TextureGrant) {
        self.budget.set_limit(owner, grant.bytes, usize::try_from(grant.count).unwrap_or(usize::MAX));
    }

    /// Forgets the textures and the texture budget of a mod that left, and marks its copy buffers for release at
    /// the next compose. The last frame's layers may still point into them, so they are not freed here.
    pub fn free_owner(&mut self, owner: u8) {
        self.budget.free_owner(owner);
        self.budget.clear_limit(owner);
        if let Some(slot) = Self::slot(owner) {
            self.released |= 1 << slot;
        }
    }

    // Releases the copy buffers of every owner marked by `free_owner`. Runs before any layer of this compose is
    // built, so nothing handed out points into them any more.
    fn release_marked(&mut self, telemetry: &Telemetry) {
        while self.released != 0 {
            let slot = self.released.trailing_zeros() as usize;
            self.released &= self.released - 1;
            for buffer in Self::buffers(slot) {
                self.vertices[buffer] = Vec::new();
                self.indices[buffer] = Vec::new();
                self.cmds[buffer] = Vec::new();
                self.deltas[buffer] = Vec::new();
                self.bytes[buffer] = Vec::new();
                self.freed[buffer] = Vec::new();
            }
            if let Ok(index) = u8::try_from(slot) {
                telemetry.record_retained_bytes(ModIndex(index), 0);
            }
        }
    }

    /// The bytes of host memory the copy buffers of all layers of `owner` keep now.
    pub fn retained_bytes(&self, owner: u8) -> u64 {
        let Some(slot) = Self::slot(owner) else {
            return 0;
        };
        let bytes: usize = Self::buffers(slot)
            .map(|buffer| {
                self.vertices[buffer].capacity() * size_of::<AmmVertex>()
                    + self.indices[buffer].capacity() * size_of::<u32>()
                    + self.cmds[buffer].capacity() * size_of::<AmmDrawCmd>()
                    + self.deltas[buffer].capacity() * size_of::<AmmTextureDelta>()
                    + self.bytes[buffer].capacity()
                    + self.freed[buffer].capacity() * size_of::<u64>()
            })
            .sum();
        bytes as u64
    }

    /// Forgets every texture after the device dropped them all.
    pub fn textures_lost(&mut self) {
        self.budget.free_all();
    }

    fn finish(&self, rt: &HostRuntime, cursor: AmmCursorShape) -> AmmHostFrame {
        let snapshot = rt.manager().snapshot();
        AmmHostFrame {
            layers: self.layers.as_ptr(),
            layer_count: u32::try_from(self.layers.len()).unwrap_or(0),
            freed_owners: rt.shared().take_freed(),
            cursor_free: snapshot.cursor_free,
            cursor,
            hide_vanilla_hud: snapshot.hide_vanilla_hud,
            game_picture: self.game_picture,
            ..AmmHostFrame::empty()
        }
    }

    // Runs the frame of one layer of a mod and adds the layer. Returns the cursor of its reply and whether the
    // pointer is over the drawn layer. A mod whose earlier layer of this compose failed too often is not called again.
    fn client_layer(
        &mut self,
        rt: &mut HostRuntime,
        entry: &LayerEntry,
        input: &AmmHostFrameInput,
        telemetry: &Telemetry,
    ) -> Option<(AmmCursorShape, bool)> {
        let index = entry.mod_index;
        let owner = usize::from(index.0);
        let failed = rt.manager().registry().get(index).is_none_or(|registered| registered.failed);
        if owner >= ModMask::CAPACITY || failed {
            return None;
        }
        let mut flags = FrameInputData::ALLOW_POINTER | FrameInputData::ALLOW_KEYBOARD;
        if rt.manager().arbiter().holds(index, Capability::Input) || rt.manager().keyboard_owner() == Some(index) {
            flags |= FrameInputData::ALLOW_NAV_KEYS;
        }
        let started = rt.manager().config().telemetry.then(Instant::now);
        let (status, out) = rt.call_frame(index, input, (entry.kind, entry.allowed), flags)?;
        let Some(reply) = Self::reply(status, &out) else {
            self.fail_frame(rt, index, input.now_ms);
            return None;
        };
        let counts = (out.mesh.vertex_count, out.mesh.index_count, out.mesh.cmd_count);
        self.used[owner].sent_vertices += out.mesh.vertex_count;
        let draw = self.used[owner].fits_draw(counts, (self.vertex_limit, self.index_limit, self.cmd_limit));
        if !draw {
            telemetry.record_vertex_skip();
        }
        let buffer = Self::buffer(owner, entry.kind);
        let picture = rt.manager().arbiter().holds(index, Capability::Overlay);
        // SAFETY: the mod's frame contract keeps every pointer of its mesh view valid until its next frame call;
        // the view is only read here, under the host lock, and everything kept is copied
        let Some(mesh) = (unsafe { self.sanitize(index.0, buffer, &out.mesh, input, telemetry, (draw, picture)) })
        else {
            self.fail_frame(rt, index, input.now_ms);
            return None;
        };
        if draw {
            self.used[owner].add_draw(counts);
        }
        self.succeeded |= 1 << owner;
        rt.manager_mut().record_frame_reply(index, entry.kind, reply);
        if let Some(started) = started {
            let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let used = &mut self.used[owner];
            used.frame_ns = used.frame_ns.saturating_add(elapsed);
            telemetry.record_frame(index, used.frame_ns, used.sent_vertices as u64);
        }
        telemetry.record_texture_bytes(index, self.budget.used(index.0));
        telemetry.record_retained_bytes(index, self.retained_bytes(index.0));
        self.layers.push(AmmHostLayer { owner: index.0, mesh });
        let (x, y) = rt.pointer();
        let [min_x, min_y, max_x, max_y] = reply.layer_rect;
        let under = draw && x >= min_x && x < max_x && y >= min_y && y < max_y;
        Some((AmmCursorShape::from(reply.cursor), under))
    }

    // Ends the run of failures of each mod whose layers all succeeded in this compose.
    fn end_runs(&self, rt: &mut HostRuntime) {
        let mut ended = self.succeeded & !self.failed;
        while ended != 0 {
            let slot = ended.trailing_zeros();
            ended &= ended - 1;
            if let Ok(slot) = u8::try_from(slot) {
                rt.manager_mut().record_frame_success(ModIndex(slot));
            }
        }
    }

    // Counts a failed layer as a failure of the mod's frame, once per compose whatever number of its layers fail.
    // A layer of the same compose that succeeds does not end the run of failures.
    fn fail_frame(&mut self, rt: &mut HostRuntime, index: ModIndex, now_ms: u64) {
        let bit = 1u32.checked_shl(u32::from(index.0)).unwrap_or(0);
        if self.failed & bit != 0 {
            return;
        }
        self.failed |= bit;
        rt.manager_mut().record_frame_failure(index, now_ms);
    }

    fn panel_layer(&mut self, rt: &mut HostRuntime, input: &AmmHostFrameInput, telemetry: &Telemetry) {
        let mut actions = std::mem::take(&mut self.panel_actions);
        let mut mesh = std::mem::take(&mut self.panel_mesh);
        rt.run_panel(input, &mut mesh, &mut actions);
        rt.apply_panel_actions(&actions);
        let view = AmmMeshView::from(&mesh);
        // SAFETY: the view points into `mesh`, a local that is not changed while the view is read
        let sanitized = unsafe {
            self.sanitize(AmmHostLayer::MANAGER, Self::MANAGER_BUFFER, &view, input, telemetry, (true, false))
        };
        let pushed = sanitized.is_some();
        match sanitized {
            Some(mesh) => self.layers.push(AmmHostLayer { owner: AmmHostLayer::MANAGER, mesh }),
            None => curia::warn!("manager panel mesh failed its own check, panel not drawn"),
        }
        if !self.panel_logged && rt.manager().snapshot().panel_open {
            self.panel_logged = true;
            curia::debug!("panel composed", {
                vertices: view.vertex_count,
                indices: view.index_count,
                draw_cmds: view.cmd_count,
                texture_deltas: view.delta_count,
                texture_bytes: view.byte_count,
                layer_pushed: pushed,
            });
        }
        self.panel_mesh = mesh;
        self.panel_actions = actions;
    }

    // The reply of a frame callback that returned Ok and valid bytes.
    fn reply(status: i32, out: &AmmFrameOutput) -> Option<FrameReplyData> {
        if AmmStatus::try_from(status) != Ok(AmmStatus::Ok) {
            return None;
        }
        FrameReplyData::try_from(out).ok()
    }

    // Checks a mesh view and copies what the overlay needs into the layer's buffers `buffer`: with `draw`, the
    // vertices, indices and commands, keeping a game picture command only with `picture`; always the deltas the
    // budget admitted with their bytes, and the freed ids. Returns a view of those copies. `None` when a pointer, a
    // command, a delta or an index is bad.
    //
    // # Safety
    //
    // Every pointer of `view` is null or valid for its count for the call.
    unsafe fn sanitize(
        &mut self,
        owner: u8,
        buffer: usize,
        view: &AmmMeshView,
        input: &AmmHostFrameInput,
        telemetry: &Telemetry,
        (draw, picture): (bool, bool),
    ) -> Option<AmmMeshView> {
        // SAFETY: the caller guarantees every pointer and count of the view
        let Some((vertices, indices, cmds, deltas, bytes, freed)) = (unsafe { Self::slices(view) }) else {
            self.note_refusal(owner, "mesh pointer null or misaligned");
            return None;
        };
        let checked_cmds = cmds.iter().all(|cmd| Self::cmd_is_valid(cmd, indices.len()));
        let checked_deltas = deltas.iter().all(|delta| Self::delta_is_valid(delta, bytes.len()));
        // the overlay draws every layer from one shared vertex buffer, so an index past this mesh's own vertices
        // would read another layer's
        let checked_indices = indices.iter().all(|&index| (index as usize) < vertices.len());
        if !checked_cmds || !checked_deltas || !checked_indices {
            let reason = if !checked_cmds {
                "draw command invalid"
            } else if !checked_deltas {
                "texture delta invalid"
            } else {
                "index past the vertices"
            };
            self.note_refusal(owner, reason);
            return None;
        }
        let slot = Self::slot(owner)?;
        if buffer >= Self::BUFFERS {
            return None;
        }
        self.copy_draw(buffer, (draw, picture), (vertices, indices, cmds), input);
        self.copy_textures((buffer, slot), owner, (deltas, bytes, freed), telemetry);
        Some(AmmMeshView {
            vertices: self.vertices[buffer].as_ptr(),
            vertex_count: self.vertices[buffer].len(),
            indices: self.indices[buffer].as_ptr(),
            index_count: self.indices[buffer].len(),
            cmds: self.cmds[buffer].as_ptr(),
            cmd_count: self.cmds[buffer].len(),
            deltas: self.deltas[buffer].as_ptr(),
            delta_count: self.deltas[buffer].len(),
            bytes: self.bytes[buffer].as_ptr(),
            byte_count: self.bytes[buffer].len(),
            freed: self.freed[buffer].as_ptr(),
            freed_count: self.freed[buffer].len(),
        })
    }

    fn copy_draw(
        &mut self,
        buffer: usize,
        (draw, picture): (bool, bool),
        (vertices, indices, cmds): (&[AmmVertex], &[u32], &[AmmDrawCmd]),
        input: &AmmHostFrameInput,
    ) {
        self.vertices[buffer].clear();
        self.indices[buffer].clear();
        self.cmds[buffer].clear();
        if !draw {
            return;
        }
        self.vertices[buffer].extend_from_slice(vertices);
        self.indices[buffer].extend_from_slice(indices);
        for cmd in cmds {
            let shows_picture = cmd.texture == GamePicture::HOST_TEXTURE;
            if shows_picture && !picture {
                continue;
            }
            if let Some(clip) = Self::clip(cmd.clip, input.width, input.height) {
                self.cmds[buffer].push(AmmDrawCmd { clip, ..*cmd });
                self.game_picture |= shows_picture;
            }
        }
    }

    // Admits each delta against the budget and copies the admitted ones with their bytes into the layer's buffers,
    // then frees the freed ids. The per-frame limits count what the owner's earlier layers of this compose kept.
    fn copy_textures(
        &mut self,
        (buffer, slot): (usize, usize),
        owner: u8,
        (deltas, bytes, freed): (&[AmmTextureDelta], &[u8], &[u64]),
        telemetry: &Telemetry,
    ) {
        let used = self.used[slot];
        let (kept_deltas, kept_bytes) = (&mut self.deltas[buffer], &mut self.bytes[buffer]);
        kept_deltas.clear();
        kept_bytes.clear();
        for delta in deltas {
            let Ok(core) = TextureDelta::try_from(delta) else {
                Self::note_refusal_in(&mut self.refusal_logged, owner, "texture delta not convertible");
                continue;
            };
            let Ok(offset) = u32::try_from(kept_bytes.len()) else {
                continue;
            };
            let uploaded = used.bytes + kept_bytes.len() as u64 + u64::from(delta.byte_len);
            let sent = used.deltas + kept_deltas.len();
            let over = delta.byte_len == 0 || uploaded > self.upload_limit || sent >= self.delta_limit;
            if over || !self.budget.admit(owner, &core) {
                let reason = if over { "upload or delta limit" } else { "texture budget refused" };
                Self::note_refusal_in(&mut self.refusal_logged, owner, reason);
                telemetry.record_texture_refusal();
                continue;
            }
            let start = delta.byte_offset as usize;
            kept_bytes.extend_from_slice(&bytes[start..start + delta.byte_len as usize]);
            kept_deltas.push(AmmTextureDelta { byte_offset: offset, ..*delta });
        }
        let (kept_delta_count, kept_byte_count) = (kept_deltas.len(), kept_bytes.len() as u64);
        let kept_freed = &freed[..freed.len().min(self.freed_limit.saturating_sub(used.freed))];
        if kept_freed.len() < freed.len() {
            telemetry.record_freed_drops((freed.len() - kept_freed.len()) as u64);
        }
        self.freed[buffer].clear();
        self.freed[buffer].extend_from_slice(kept_freed);
        let used = &mut self.used[slot];
        used.deltas += kept_delta_count;
        used.bytes += kept_byte_count;
        used.freed += kept_freed.len();
        for id in kept_freed {
            self.budget.free(owner, *id);
        }
    }

    // Writes one line per open of the panel when something of the manager layer is refused.
    fn note_refusal(&mut self, owner: u8, reason: &str) {
        Self::note_refusal_in(&mut self.refusal_logged, owner, reason);
    }

    fn note_refusal_in(logged: &mut bool, owner: u8, reason: &str) {
        if owner != AmmHostLayer::MANAGER || *logged {
            return;
        }
        *logged = true;
        curia::info!("panel layer refused", { reason: reason });
    }

    #[allow(clippy::type_complexity)]
    unsafe fn slices<'a>(
        view: &AmmMeshView,
    ) -> Option<(&'a [AmmVertex], &'a [u32], &'a [AmmDrawCmd], &'a [AmmTextureDelta], &'a [u8], &'a [u64])> {
        // SAFETY: the caller guarantees every pointer and count of the view
        unsafe {
            Some((
                Self::slice(view.vertices, view.vertex_count)?,
                Self::slice(view.indices, view.index_count)?,
                Self::slice(view.cmds, view.cmd_count)?,
                Self::slice(view.deltas, view.delta_count)?,
                Self::slice(view.bytes, view.byte_count)?,
                Self::slice(view.freed, view.freed_count)?,
            ))
        }
    }

    // A slice from a pointer and a count. A count of 0 is empty; a null or misaligned pointer is refused.
    //
    // # Safety
    //
    // A non-null, aligned `ptr` points at `len` valid values for `'a`.
    unsafe fn slice<'a, T>(ptr: *const T, len: usize) -> Option<&'a [T]> {
        if len == 0 {
            return Some(&[]);
        }
        if ptr.is_null() || !ptr.is_aligned() {
            return None;
        }
        // SAFETY: the caller guarantees len values at a non-null, aligned ptr for 'a
        Some(unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    fn cmd_is_valid(cmd: &AmmDrawCmd, index_count: usize) -> bool {
        let Ok(core) = DrawCmd::try_from(cmd) else {
            return false;
        };
        let end = u64::from(core.index_offset) + u64::from(core.index_count);
        core.shader <= DrawCmd::SHADER_PICTURE && end <= index_count as u64
    }

    fn delta_is_valid(delta: &AmmTextureDelta, byte_count: usize) -> bool {
        let Ok(core) = TextureDelta::try_from(delta) else {
            return false;
        };
        let needed = u64::from(core.width) * u64::from(core.height) * 4;
        let end = u64::from(core.byte_offset) + u64::from(core.byte_len);
        end <= byte_count as u64 && u64::from(core.byte_len) >= needed
    }

    // The clip rect inside the back buffer, rounded to whole pixels, or `None` when it is NaN or empty.
    fn clip(clip: [f32; 4], width: u32, height: u32) -> Option<[f32; 4]> {
        if clip.iter().any(|value| value.is_nan()) {
            return None;
        }
        let (width, height) = (width as f32, height as f32);
        let min_x = clip[0].clamp(0.0, width).round();
        let min_y = clip[1].clamp(0.0, height).round();
        let max_x = clip[2].clamp(0.0, width).round();
        let max_y = clip[3].clamp(0.0, height).round();
        (max_x > min_x && max_y > min_y).then_some([min_x, min_y, max_x, max_y])
    }

    // The copy buffers of one layer: one per layer kind of each mod slot, Hud then Overlay, and one for the manager.
    fn buffer(owner: usize, kind: LayerKind) -> usize {
        LayerKind::COUNT * owner + kind.slot()
    }

    // The copy buffers of every layer of the owner in `slot`.
    fn buffers(slot: usize) -> std::ops::Range<usize> {
        if slot == Self::MANAGER_SLOT {
            Self::MANAGER_BUFFER..Self::BUFFERS
        } else {
            LayerKind::COUNT * slot..LayerKind::COUNT * (slot + 1)
        }
    }

    fn slot(owner: u8) -> Option<usize> {
        match owner {
            AmmHostLayer::MANAGER => Some(Self::MANAGER_SLOT),
            owner if usize::from(owner) < ModMask::CAPACITY => Some(usize::from(owner)),
            _ => None,
        }
    }
}
