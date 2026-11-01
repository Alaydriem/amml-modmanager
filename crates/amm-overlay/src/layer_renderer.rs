use std::ops::Range;

use amm_abi::types::{AmmDrawCmd, AmmSamplerKind, AmmVertex};
use amm_ffi::types::{AMM_HOST_GAME_PICTURE_TEXTURE, AmmHostFrame, AmmHostLayer};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_FLAG, D3D11_BIND_INDEX_BUFFER, D3D11_BIND_VERTEX_BUFFER, D3D11_BLEND_DESC,
    D3D11_BLEND_INV_DEST_ALPHA, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BUFFER_DESC,
    D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_ALWAYS, D3D11_CPU_ACCESS_WRITE, D3D11_CULL_NONE,
    D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_WRITE_MASK_ZERO, D3D11_FILL_SOLID, D3D11_FILTER,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_FLOAT32_MAX, D3D11_INPUT_ELEMENT_DESC,
    D3D11_INPUT_PER_VERTEX_DATA, D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE, D3D11_RASTERIZER_DESC,
    D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT, ID3D11BlendState, ID3D11Buffer, ID3D11DepthStencilState, ID3D11Device, ID3D11DeviceContext,
    ID3D11InputLayout, ID3D11PixelShader, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R32G32_FLOAT,
};
use windows::core::{BOOL, s};

use crate::draw_report::DrawReport;
use crate::game_picture::GamePicture;
use crate::mesh_slices::MeshSlices;
use crate::overlay_error::OverlayError;
use crate::textures::TextureRegistry;

const _: () = assert!(size_of::<AmmVertex>() == 20, "AmmVertex must match the input layout");

/// Draws the layers of one `AmmHostFrame` with D3D11, bottom to top, in one or more ranges. Render thread only.
/// Works on the game's own D3D11 device and on the D3D11On12 device of the D3D12 path; the caller keeps the
/// context state it needs (`StateBackup`). Each draw command picks its pixel shader by `shader` and its sampler
/// by `sampler`; a command with an unknown shader or sampler is skipped and logged once. Owns the layer textures
/// and the game picture they may draw.
pub struct LayerRenderer {
    device: ID3D11Device,
    vertex_shader: ID3D11VertexShader,
    pixel_shaders: [ID3D11PixelShader; Self::SHADER_COUNT],
    layout: ID3D11InputLayout,
    constants: Option<ID3D11Buffer>,
    blend: ID3D11BlendState,
    raster: ID3D11RasterizerState,
    depth: ID3D11DepthStencilState,
    // linear then point, each kept as a one-element slice to bind
    samplers: [[Option<ID3D11SamplerState>; 1]; 2],
    // grow-only, shared by every layer of a range
    vertices: Option<ID3D11Buffer>,
    indices: Option<ID3D11Buffer>,
    vertex_capacity: u32,
    index_capacity: u32,
    textures: TextureRegistry,
    picture: GamePicture,
    shader_logged: bool,
    sampler_logged: bool,
    oversize_logged: bool,
    // the manager layer's drawn and skipped lines were written for this open
    manager_drawn_logged: bool,
    manager_skip_logged: bool,
}

impl LayerRenderer {
    const VERTEX_CODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/vs_main.cso"));
    const MAIN_CODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ps_main.cso"));
    const PICTURE_CODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ps_picture.cso"));
    // the pixel shaders a draw command can name, by index: DrawCmd::SHADER_EGUI, SHADER_USER and SHADER_PICTURE.
    // A user texture is drawn as egui draws its own
    const PIXEL_CODE: [&'static [u8]; 3] = [Self::MAIN_CODE, Self::MAIN_CODE, Self::PICTURE_CODE];
    const SHADER_COUNT: usize = Self::PIXEL_CODE.len();

    const VERTEX_STRIDE: u32 = size_of::<AmmVertex>() as u32;
    const INDEX_STRIDE: u32 = size_of::<u32>() as u32;
    const INITIAL_VERTICES: u32 = 16_384;
    const INITIAL_INDICES: u32 = 32_768;
    const CONSTANT_FLOATS: usize = 4;
    // 16 mods at 262,144 vertices each plus the manager fit far below this; a frame past it is not drawn
    const MAX_BUFFER_ELEMENTS: u32 = 16 * 1024 * 1024;

    /// Makes the fixed pipeline objects on `device`.
    pub fn new(device: &ID3D11Device) -> Result<Self, OverlayError> {
        let vertex_shader = Self::vertex_shader(device)?;
        let pixel_shaders = [
            Self::pixel_shader(device, Self::PIXEL_CODE[0])?,
            Self::pixel_shader(device, Self::PIXEL_CODE[1])?,
            Self::pixel_shader(device, Self::PIXEL_CODE[2])?,
        ];
        Ok(Self {
            device: device.clone(),
            vertex_shader,
            pixel_shaders,
            layout: Self::input_layout(device)?,
            constants: Some(Self::constant_buffer(device)?),
            blend: Self::blend_state(device)?,
            raster: Self::rasterizer_state(device)?,
            depth: Self::depth_state(device)?,
            samplers: [
                [Some(Self::sampler(device, D3D11_FILTER_MIN_MAG_MIP_LINEAR)?)],
                [Some(Self::sampler(device, D3D11_FILTER_MIN_MAG_MIP_POINT)?)],
            ],
            vertices: None,
            indices: None,
            vertex_capacity: 0,
            index_capacity: 0,
            textures: TextureRegistry::new(),
            picture: GamePicture::new(),
            shader_logged: false,
            sampler_logged: false,
            oversize_logged: false,
            manager_drawn_logged: false,
            manager_skip_logged: false,
        })
    }

    /// Applies the texture deltas of `layers` of `frame`, draws them into `target` of `size` pixels, then frees
    /// the textures each of them lists. Binds `target` and leaves the pipeline state set; called for one or more
    /// ranges of a frame, in layer order. A range past the frame's layers is cut to it.
    ///
    /// # Safety
    ///
    /// `frame` is a host frame as `amm_host_frame` fills it: its layer pointer and every pointer of each layer's
    /// mesh are null or point to their counts of live elements for the call.
    pub unsafe fn draw(
        &mut self,
        ctx: &ID3D11DeviceContext,
        target: &ID3D11RenderTargetView,
        size: (u32, u32),
        frame: &AmmHostFrame,
        layers: Range<usize>,
    ) -> DrawReport {
        let mut report = DrawReport::default();
        // SAFETY: the caller vouches for the frame's layer pointer
        let Some(all) = (unsafe { MeshSlices::layers(frame) }) else {
            return report;
        };
        let end = layers.end.min(all.len());
        if layers.start >= end {
            return report;
        }
        let range = &all[layers.start..end];
        for layer in range {
            // SAFETY: the caller vouches for the mesh pointers of every layer
            let mesh = unsafe { MeshSlices::new(&layer.mesh) };
            self.textures.apply(&self.device, ctx, layer.owner, mesh.deltas, mesh.bytes);
        }
        // SAFETY: as above
        unsafe { self.draw_geometry(ctx, target, size, range, &mut report) };
        for layer in range {
            // SAFETY: as above
            let mesh = unsafe { MeshSlices::new(&layer.mesh) };
            self.textures.free(layer.owner, mesh.freed);
        }
        report
    }

    /// The once-per-frame part of drawing `frame`: the manager layer's open and close logs and the freed owners.
    /// Call it once for each frame, before any `draw` of that frame, also for a frame with no layers.
    ///
    /// # Safety
    ///
    /// As for `draw`.
    pub unsafe fn begin_frame(&mut self, frame: &AmmHostFrame) {
        // SAFETY: the caller vouches for the frame's pointers
        let Some(layers) = (unsafe { MeshSlices::layers(frame) }) else {
            return;
        };
        // SAFETY: as above
        unsafe { self.note_manager_drawn(layers) };
        self.textures.free_owners(frame.freed_owners);
    }

    /// The index of the first manager layer of `frame`, or its layer count when it has none. The layers before it
    /// are the mod layers.
    ///
    /// # Safety
    ///
    /// As for `draw`.
    pub unsafe fn mod_layer_end(frame: &AmmHostFrame) -> usize {
        // SAFETY: the caller vouches for the frame's layer pointer
        match unsafe { MeshSlices::layers(frame) } {
            Some(layers) => {
                layers.iter().position(|layer| layer.owner == AmmHostLayer::MANAGER).unwrap_or(layers.len())
            }
            None => 0,
        }
    }

    /// Re-arms the once-per-open logs of the manager layer; called for a frame that has no manager layer.
    pub fn manager_closed(&mut self) {
        self.manager_drawn_logged = false;
        self.manager_skip_logged = false;
        self.textures.manager_closed();
    }

    /// Drops every layer texture and the game picture, for a resize or a device loss. The caller then tells the
    /// manager its textures are lost.
    pub fn release_textures(&mut self) {
        self.textures.drop_all();
        self.picture.release();
    }

    pub fn textures(&self) -> &TextureRegistry {
        &self.textures
    }

    pub fn textures_mut(&mut self) -> &mut TextureRegistry {
        &mut self.textures
    }

    pub fn game_picture(&self) -> &GamePicture {
        &self.picture
    }

    pub fn game_picture_mut(&mut self) -> &mut GamePicture {
        &mut self.picture
    }

    unsafe fn draw_geometry(
        &mut self,
        ctx: &ID3D11DeviceContext,
        target: &ID3D11RenderTargetView,
        size: (u32, u32),
        layers: &[AmmHostLayer],
        report: &mut DrawReport,
    ) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        let (width, height) = (size.0 as f32, size.1 as f32);
        let (mut vertex_count, mut index_count) = (0usize, 0usize);
        for layer in layers {
            // SAFETY: the caller of draw vouches for the mesh pointers
            let mesh = unsafe { MeshSlices::new(&layer.mesh) };
            if mesh.usable() {
                vertex_count += mesh.vertices.len();
                index_count += mesh.indices.len();
            }
        }
        if vertex_count == 0 || index_count == 0 {
            return;
        }
        let fits = Self::ensure_buffer(
            &self.device,
            &mut self.vertices,
            &mut self.vertex_capacity,
            vertex_count,
            Self::VERTEX_STRIDE,
            D3D11_BIND_VERTEX_BUFFER,
            Self::INITIAL_VERTICES,
        ) && Self::ensure_buffer(
            &self.device,
            &mut self.indices,
            &mut self.index_capacity,
            index_count,
            Self::INDEX_STRIDE,
            D3D11_BIND_INDEX_BUFFER,
            Self::INITIAL_INDICES,
        );
        if !fits {
            if !self.oversize_logged {
                curia::warn!("frame geometry does not fit the layer buffers, frame not drawn", {
                    vertices: vertex_count,
                    indices: index_count
                });
            }
            self.oversize_logged = true;
            report.geometry_dropped = true;
            return;
        }
        // SAFETY: as above; the buffers hold the counted vertices and indices
        if !unsafe { self.upload_geometry(ctx, layers) } || !self.upload_constants(ctx, width, height) {
            report.geometry_dropped = true;
            return;
        }
        self.bind_pipeline(ctx, target, width, height);
        // SAFETY: as above
        unsafe { self.draw_layers(ctx, layers, width, height, report) };
    }

    // grows only: a buffer is made again, at least twice as large, when a frame needs more than it holds
    fn ensure_buffer(
        device: &ID3D11Device,
        buffer: &mut Option<ID3D11Buffer>,
        capacity: &mut u32,
        needed: usize,
        stride: u32,
        bind: D3D11_BIND_FLAG,
        initial: u32,
    ) -> bool {
        if buffer.is_some() && needed <= *capacity as usize {
            return true;
        }
        if needed > Self::MAX_BUFFER_ELEMENTS as usize {
            return false;
        }
        let grown = (needed as u32).max(initial).max((*capacity * 2).min(Self::MAX_BUFFER_ELEMENTS));
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: grown * stride,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        *buffer = None;
        *capacity = 0;
        let mut made: Option<ID3D11Buffer> = None;
        // SAFETY: desc lives for the call and the buffer has no initial data
        if unsafe { device.CreateBuffer(&desc, None, Some(&mut made)) }.is_err() || made.is_none() {
            return false;
        }
        *buffer = made;
        *capacity = grown;
        true
    }

    // the vertices and indices of each layer go into one shared pair of buffers, one after the other in layer
    // order
    unsafe fn upload_geometry(&self, ctx: &ID3D11DeviceContext, layers: &[AmmHostLayer]) -> bool {
        let (Some(vertices), Some(indices)) = (&self.vertices, &self.indices) else {
            return false;
        };
        let mut vertex_map = D3D11_MAPPED_SUBRESOURCE::default();
        let mut index_map = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the buffer is a live dynamic buffer with CPU write access; the out struct is writable
        if unsafe { ctx.Map(vertices, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut vertex_map)) }.is_err() {
            return false;
        }
        // SAFETY: as above
        if unsafe { ctx.Map(indices, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut index_map)) }.is_err() {
            // SAFETY: the vertex buffer was mapped above
            unsafe { ctx.Unmap(vertices, 0) };
            return false;
        }
        let mut vertex_out = vertex_map.pData.cast::<AmmVertex>();
        let mut index_out = index_map.pData.cast::<u32>();
        for layer in layers {
            // SAFETY: the caller of draw vouches for the mesh pointers
            let mesh = unsafe { MeshSlices::new(&layer.mesh) };
            if !mesh.usable() {
                continue;
            }
            // SAFETY: the mapped buffers hold at least the vertices and indices counted over these layers, and a
            // mapped buffer is aligned for any element type
            unsafe {
                std::ptr::copy_nonoverlapping(mesh.vertices.as_ptr(), vertex_out, mesh.vertices.len());
                std::ptr::copy_nonoverlapping(mesh.indices.as_ptr(), index_out, mesh.indices.len());
                vertex_out = vertex_out.add(mesh.vertices.len());
                index_out = index_out.add(mesh.indices.len());
            }
        }
        // SAFETY: both buffers were mapped above
        unsafe {
            ctx.Unmap(indices, 0);
            ctx.Unmap(vertices, 0);
        }
        true
    }

    fn upload_constants(&self, ctx: &ID3D11DeviceContext, width: f32, height: f32) -> bool {
        let Some(constants) = &self.constants else {
            return false;
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the buffer is a live dynamic buffer with CPU write access; the out struct is writable
        if unsafe { ctx.Map(constants, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)) }.is_err() {
            return false;
        }
        let values: [f32; Self::CONSTANT_FLOATS] = [width, height, 0.0, 0.0];
        // SAFETY: the mapped buffer is CONSTANT_FLOATS floats long and aligned; it is unmapped right after
        unsafe {
            std::ptr::copy_nonoverlapping(values.as_ptr(), mapped.pData.cast::<f32>(), values.len());
            ctx.Unmap(constants, 0);
        }
        true
    }

    fn bind_pipeline(&self, ctx: &ID3D11DeviceContext, target: &ID3D11RenderTargetView, width: f32, height: f32) {
        let viewport =
            D3D11_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: width, Height: height, MinDepth: 0.0, MaxDepth: 1.0 };
        let stride = Self::VERTEX_STRIDE;
        let offset = 0u32;
        let blend_factor = [0.0f32; 4];
        // SAFETY: every object bound is live and owned by self or the caller; each pointer passed lives for its
        // call
        unsafe {
            ctx.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            ctx.RSSetViewports(Some(&[viewport]));
            ctx.RSSetState(&self.raster);
            ctx.IASetInputLayout(&self.layout);
            ctx.IASetVertexBuffers(0, 1, Some(&self.vertices), Some(&stride), Some(&offset));
            ctx.IASetIndexBuffer(self.indices.as_ref(), DXGI_FORMAT_R32_UINT, 0);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.VSSetShader(&self.vertex_shader, None);
            ctx.VSSetConstantBuffers(0, Some(std::slice::from_ref(&self.constants)));
            ctx.GSSetShader(None, None);
            ctx.HSSetShader(None, None);
            ctx.DSSetShader(None, None);
            ctx.OMSetBlendState(&self.blend, Some(&blend_factor), 0xffff_ffff);
            ctx.OMSetDepthStencilState(&self.depth, 0);
        }
    }

    unsafe fn draw_layers(
        &mut self,
        ctx: &ID3D11DeviceContext,
        layers: &[AmmHostLayer],
        width: f32,
        height: f32,
        report: &mut DrawReport,
    ) {
        let mut index_base = 0u32;
        let mut vertex_base = 0i32;
        for layer in layers {
            // SAFETY: the caller of draw vouches for the mesh pointers
            let mesh = unsafe { MeshSlices::new(&layer.mesh) };
            if !mesh.usable() {
                continue;
            }
            self.draw_layer(ctx, layer.owner, &mesh, index_base, vertex_base, (width, height), report);
            index_base += mesh.indices.len() as u32;
            vertex_base += mesh.vertices.len() as i32;
        }
        // the texture view must not stay bound past the draw: a freed texture would live on through the binding
        // SAFETY: unbinding slot 0 takes no object
        unsafe { ctx.PSSetShaderResources(0, Some(&[None])) };
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_layer(
        &mut self,
        ctx: &ID3D11DeviceContext,
        owner: u8,
        mesh: &MeshSlices<'_>,
        index_base: u32,
        vertex_base: i32,
        size: (f32, f32),
        report: &mut DrawReport,
    ) {
        for cmd in mesh.cmds {
            if !Self::cmd_in_range(cmd, mesh.indices.len()) {
                report.skipped_range += 1;
                self.note_manager_skip(owner, "command index range outside the mesh");
                continue;
            }
            let (Some(shader), Some(sampler)) = (self.shader_for(cmd), self.sampler_for(cmd)) else {
                report.skipped_pipeline += 1;
                self.note_manager_skip(owner, "unknown shader or sampler");
                continue;
            };
            let view = match cmd.texture {
                // the manager keeps a game picture command only in a layer of the Overlay holder
                AMM_HOST_GAME_PICTURE_TEXTURE => self.picture.bound_view(),
                id => self.textures.bound_view(owner, id),
            };
            let Some(view) = view else {
                report.skipped_texture += 1;
                self.note_manager_skip(owner, "texture missing from the registry");
                continue;
            };
            let Some(rect) = Self::scissor_for(cmd, size.0, size.1) else {
                report.skipped_scissor += 1;
                self.note_manager_skip(owner, "scissor empty after clamp");
                continue;
            };
            // SAFETY: the shader, sampler and view are live objects of this device; the index range was checked
            // against the mesh, whose indices sit at index_base in the bound buffer
            unsafe {
                ctx.RSSetScissorRects(Some(&[rect]));
                ctx.PSSetShader(&self.pixel_shaders[shader], None);
                ctx.PSSetSamplers(0, Some(&self.samplers[sampler]));
                ctx.PSSetShaderResources(0, Some(view));
                ctx.DrawIndexed(cmd.index_count, index_base + cmd.index_offset, vertex_base);
            }
            report.drawn += 1;
        }
    }

    fn cmd_in_range(cmd: &AmmDrawCmd, index_count: usize) -> bool {
        let offset = cmd.index_offset as usize;
        cmd.index_count != 0 && offset <= index_count && cmd.index_count as usize <= index_count - offset
    }

    fn shader_for(&mut self, cmd: &AmmDrawCmd) -> Option<usize> {
        let shader = usize::from(cmd.shader);
        if shader < Self::SHADER_COUNT {
            return Some(shader);
        }
        if !self.shader_logged {
            curia::warn!("draw command names an unknown pixel shader, skipped", { shader: cmd.shader });
        }
        self.shader_logged = true;
        None
    }

    fn sampler_for(&mut self, cmd: &AmmDrawCmd) -> Option<usize> {
        if cmd.sampler == AmmSamplerKind::Linear as u8 {
            return Some(0);
        }
        if cmd.sampler == AmmSamplerKind::Nearest as u8 {
            return Some(1);
        }
        if !self.sampler_logged {
            curia::warn!("draw command names an unknown sampler, skipped", { sampler: cmd.sampler });
        }
        self.sampler_logged = true;
        None
    }

    // the clip rectangle in pixels, cut to the current target in case it was resized after the frame was
    // composed; None when nothing of it is left
    fn scissor_for(cmd: &AmmDrawCmd, width: f32, height: f32) -> Option<RECT> {
        if cmd.clip.iter().any(|edge| !edge.is_finite()) {
            return None;
        }
        let rect = RECT {
            left: cmd.clip[0].clamp(0.0, width).floor() as i32,
            top: cmd.clip[1].clamp(0.0, height).floor() as i32,
            right: cmd.clip[2].clamp(0.0, width).ceil() as i32,
            bottom: cmd.clip[3].clamp(0.0, height).ceil() as i32,
        };
        (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
    }

    // one line per panel open when a frame first carries the manager layer
    unsafe fn note_manager_drawn(&mut self, layers: &[AmmHostLayer]) {
        let Some(layer) = layers.iter().find(|layer| layer.owner == AmmHostLayer::MANAGER) else {
            self.manager_closed();
            return;
        };
        if self.manager_drawn_logged {
            return;
        }
        self.manager_drawn_logged = true;
        // SAFETY: the caller of begin_frame vouches for the mesh pointers
        let usable = unsafe { MeshSlices::new(&layer.mesh) }.usable();
        curia::debug!("manager layer drawn", {
            vertices: layer.mesh.vertex_count,
            cmds: layer.mesh.cmd_count,
            usable: usable
        });
    }

    // one line per panel open when a draw command of the manager layer is skipped
    fn note_manager_skip(&mut self, owner: u8, reason: &'static str) {
        if owner != AmmHostLayer::MANAGER || self.manager_skip_logged {
            return;
        }
        self.manager_skip_logged = true;
        curia::info!("manager draw skipped", { reason: reason });
    }

    fn vertex_shader(device: &ID3D11Device) -> Result<ID3D11VertexShader, OverlayError> {
        let mut shader = None;
        // SAFETY: the bytecode is a compiled vs_4_0 shader embedded in the binary
        let result = unsafe { device.CreateVertexShader(Self::VERTEX_CODE, None, Some(&mut shader)) };
        OverlayError::created("vertex shader", result, shader)
    }

    fn pixel_shader(device: &ID3D11Device, code: &[u8]) -> Result<ID3D11PixelShader, OverlayError> {
        let mut shader = None;
        // SAFETY: the bytecode is a compiled ps_4_0 shader embedded in the binary
        let result = unsafe { device.CreatePixelShader(code, None, Some(&mut shader)) };
        OverlayError::created("pixel shader", result, shader)
    }

    fn input_layout(device: &ID3D11Device) -> Result<ID3D11InputLayout, OverlayError> {
        let elements = [
            Self::element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, std::mem::offset_of!(AmmVertex, pos)),
            Self::element(s!("TEXCOORD"), DXGI_FORMAT_R32G32_FLOAT, std::mem::offset_of!(AmmVertex, uv)),
            Self::element(s!("COLOR"), DXGI_FORMAT_R8G8B8A8_UNORM, std::mem::offset_of!(AmmVertex, color)),
        ];
        let mut layout = None;
        // SAFETY: the element names are static strings and the bytecode carries the vertex shader's input
        // signature
        let result = unsafe { device.CreateInputLayout(&elements, Self::VERTEX_CODE, Some(&mut layout)) };
        OverlayError::created("input layout", result, layout)
    }

    fn element(
        name: windows::core::PCSTR,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        offset: usize,
    ) -> D3D11_INPUT_ELEMENT_DESC {
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: name,
            SemanticIndex: 0,
            Format: format,
            InputSlot: 0,
            AlignedByteOffset: offset as u32,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }
    }

    fn constant_buffer(device: &ID3D11Device) -> Result<ID3D11Buffer, OverlayError> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: (Self::CONSTANT_FLOATS * size_of::<f32>()) as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut buffer = None;
        // SAFETY: desc lives for the call and the buffer has no initial data
        let result = unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) };
        OverlayError::created("constant buffer", result, buffer)
    }

    fn blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState, OverlayError> {
        let mut desc = D3D11_BLEND_DESC::default();
        desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL::from(true),
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_INV_DEST_ALPHA,
            DestBlendAlpha: D3D11_BLEND_ONE,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut state = None;
        // SAFETY: desc lives for the call
        let result = unsafe { device.CreateBlendState(&desc, Some(&mut state)) };
        OverlayError::created("blend state", result, state)
    }

    fn rasterizer_state(device: &ID3D11Device) -> Result<ID3D11RasterizerState, OverlayError> {
        let desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            ScissorEnable: BOOL::from(true),
            DepthClipEnable: BOOL::from(true),
            ..Default::default()
        };
        let mut state = None;
        // SAFETY: desc lives for the call
        let result = unsafe { device.CreateRasterizerState(&desc, Some(&mut state)) };
        OverlayError::created("rasterizer state", result, state)
    }

    fn depth_state(device: &ID3D11Device) -> Result<ID3D11DepthStencilState, OverlayError> {
        let desc = D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: BOOL::from(false),
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: D3D11_COMPARISON_ALWAYS,
            StencilEnable: BOOL::from(false),
            ..Default::default()
        };
        let mut state = None;
        // SAFETY: desc lives for the call
        let result = unsafe { device.CreateDepthStencilState(&desc, Some(&mut state)) };
        OverlayError::created("depth stencil state", result, state)
    }

    fn sampler(device: &ID3D11Device, filter: D3D11_FILTER) -> Result<ID3D11SamplerState, OverlayError> {
        let desc = D3D11_SAMPLER_DESC {
            Filter: filter,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            ComparisonFunc: D3D11_COMPARISON_ALWAYS,
            MaxLOD: D3D11_FLOAT32_MAX,
            ..Default::default()
        };
        let mut state = None;
        // SAFETY: desc lives for the call
        let result = unsafe { device.CreateSamplerState(&desc, Some(&mut state)) };
        OverlayError::created("sampler state", result, state)
    }
}
