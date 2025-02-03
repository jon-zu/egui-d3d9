use egui::{epaint::Primitive, Context, TextureId};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Direct3D9::{IDirect3DDevice9, IDirect3DTexture9, D3DPT_TRIANGLELIST, D3DVIEWPORT9},
    UI::WindowsAndMessaging::GetClientRect,
};

use crate::{
    inputman::InputManager,
    mesh::{Buffers, GpuVertex, MeshDescriptor},
    set_clipboard_text,
    state::DxState,
    texman::TextureManager,
};

pub trait UIHandler {
    fn ui(&mut self, ctx: &Context);

    #[allow(unused_variables)]
    fn resolve_user_texture(&mut self, id: u64) -> Option<&IDirect3DTexture9> {
        None
    }
}

pub struct EguiDx9<H> {
    handler: H,
    hwnd: HWND,
    reactive: bool,
    input_man: InputManager,
    // get it? tEx-man? tax-man? no?
    tex_man: TextureManager,
    ctx: Context,
    buffers: Buffers,
    prims: Vec<MeshDescriptor>,
    last_idx_capacity: usize,
    last_vtx_capacity: usize,
    should_reset: bool,

    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
}

impl<H: UIHandler> EguiDx9<H> {
    ///
    /// initialize the backend.
    ///
    ///
    /// if you are using this purely as a UI, you can set `reactive` to true.
    /// this causes us to only re-draw the menu once something changes.
    ///
    /// the menu doesn't always catch these changes, so only use this if you need to.
    ///
    /// # Panics
    /// If buffers cannot be created
    pub fn init(dev: &IDirect3DDevice9, hwnd: HWND, handler: H, reactive: bool) -> Self {
        Self {
            handler,
            hwnd,
            reactive,
            tex_man: TextureManager::new(),
            input_man: InputManager::new(hwnd),
            ctx: Context::default(),
            buffers: Buffers::create_buffers(dev, 16384, 16384).expect("buffers"),
            prims: Vec::new(),
            last_idx_capacity: 0,
            last_vtx_capacity: 0,
            should_reset: false,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn pre_reset(&mut self) {
        self.buffers.delete_buffers();
        self.tex_man.deallocate_textures();

        self.should_reset = true;
    }

    /// # Panics
    /// # Errors
    /// underlying render error
    pub fn present(&mut self, dev: &IDirect3DDevice9) -> windows::core::Result<()> {
        if unsafe { dev.TestCooperativeLevel() }.is_err() {
            return Ok(());
        }

        if self.should_reset {
            self.buffers = Buffers::create_buffers(dev, 16384, 16384)?;
            self.tex_man.reallocate_textures(dev);
        }

        let output = self.ctx.run(self.input_man.collect_input(), |ctx| {
            // safe. present will never run in parallel.
            self.handler.ui(ctx);
        });

        if self.should_reset {
            self.ctx.request_repaint();
            self.should_reset = false;
        }

        if !output.textures_delta.is_empty() {
            self.tex_man
                .process_set_deltas(dev, &output.textures_delta)?;
        }

        if !output.platform_output.copied_text.is_empty() {
            let _ = set_clipboard_text(output.platform_output.copied_text);
        }

        if output.shapes.is_empty() {
            // early return, don't forget to free textures
            if !output.textures_delta.is_empty() {
                self.tex_man.process_free_deltas(&output.textures_delta);
            }
            return Ok(());
        }

        // we only need to update the buffers if we are actually changing something
        if self.ctx.has_requested_repaint() || !self.reactive {
            // TODO: old code added last len + 512
            self.vertices.clear();
            self.indices.clear();

            self.prims = self
                .ctx
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .filter_map(|prim| {
                    if let Primitive::Mesh(mesh) = prim.primitive {
                        // most definitely not the rusty way to do this.
                        // it's ugly, but its efficient.
                        if let Some((gpumesh, verts, idxs)) =
                            MeshDescriptor::from_mesh(mesh, prim.clip_rect)
                        {
                            self.vertices.extend_from_slice(&verts);
                            self.indices.extend_from_slice(&idxs);

                            Some(gpumesh)
                        } else {
                            None
                        }
                    } else {
                        panic!("paint callbacks not supported")
                    }
                })
                .collect();

            self.last_vtx_capacity = self.vertices.len();
            self.last_idx_capacity = self.indices.len();

            self.buffers.update_vertex_buffer(dev, &self.vertices)?;
            self.buffers.update_index_buffer(dev, &self.indices)?;
        }

        // back up our state so we don't mess with the game and the game doesn't mess with us.
        // i actually had the idea to use BeginStateBlock and co. to "cache" the state we set every frame,
        // and just re-applying it everytime. just setting this manually takes around 50 microseconds on my machine.
        let _state = DxState::setup(dev, self.get_viewport());

        unsafe {
            dev.SetStreamSource(
                0,
                self.buffers
                    .vtx
                    .as_ref()
                    .expect("unable to get vertex buffer"),
                0,
                std::mem::size_of::<GpuVertex>() as _,
            )?;

            dev.SetIndices(
                self.buffers
                    .idx
                    .as_ref()
                    .expect("unable to get index buffer"),
            )?;
        }

        let mut our_vtx_idx: usize = 0;
        let mut our_idx_idx: usize = 0;

        self.prims
            .iter()
            .try_for_each(|mesh: &MeshDescriptor| unsafe {
                dev.SetScissorRect(&mesh.clip)?;

                let texture = match mesh.texture_id {
                    TextureId::Managed(id) => self.tex_man.get_by_id(TextureId::Managed(id)),
                    TextureId::User(id) => self
                        .handler
                        .resolve_user_texture(id)
                        .expect("unable to resolve user texture"),
                };

                dev.SetTexture(0, texture)?;

                dev.DrawIndexedPrimitive(
                    D3DPT_TRIANGLELIST,
                    our_vtx_idx as _,
                    0,
                    mesh.vertices as _,
                    our_idx_idx as _,
                    (mesh.indices / 3usize) as _,
                )?;

                our_vtx_idx += mesh.vertices;
                our_idx_idx += mesh.indices;
                windows::core::Result::Ok(())
            })?;

        if !output.textures_delta.is_empty() {
            self.tex_man.process_free_deltas(&output.textures_delta);
        }

        Ok(())
    }

    #[inline]
    pub fn wnd_proc(&mut self, umsg: u32, wparam: WPARAM, lparam: LPARAM) {
        // safe. we only write here, and only read elsewhere.
        self.input_man.process(umsg, wparam.0, lparam.0);
    }
}

impl<T> EguiDx9<T> {
    #[allow(clippy::cast_sign_loss)]
    fn get_screen_size(&self) -> (u32, u32) {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut rect).expect("Failed to GetClientRect()");
        }
        (
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        )
    }

    fn get_viewport(&self) -> D3DVIEWPORT9 {
        let (w, h) = self.get_screen_size();
        D3DVIEWPORT9 {
            X: 0,
            Y: 0,
            Width: w as _,
            Height: h as _,
            MinZ: 0.,
            MaxZ: 1.,
        }
    }
}

impl<H> Drop for EguiDx9<H> {
    fn drop(&mut self) {
        self.buffers.delete_buffers();
        self.tex_man.deallocate_textures();
    }
}
