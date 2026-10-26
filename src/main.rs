use std::collections::HashMap;
use std::num::NonZeroU32;

use exwlshellev::keyboard::{KeyCode, PhysicalKey};
use exwlshellev::reexport::*;
use exwlshellev::*;
use tiny_skia::*;

struct Window {
    context: softbuffer::Context<DisplayWrapper>,
    buffers: HashMap<id::Id, softbuffer::Surface<DisplayWrapper, WindowWrapper>>,
}

fn draw(width: u32, height: u32) -> Pixmap {
    let mut paint1 = Paint::default();
    paint1.set_color_rgba8(50, 127, 150, 200);
    paint1.anti_alias = true;

    let mut paint2 = Paint::default();
    paint2.set_color_rgba8(220, 140, 75, 180);
    paint2.anti_alias = false;

    let path1 = {
        let mut pb = PathBuilder::new();
        pb.move_to(60.0, 60.0);
        pb.line_to(160.0, 940.0);
        pb.cubic_to(380.0, 840.0, 660.0, 800.0, 940.0, 800.0);
        pb.cubic_to(740.0, 460.0, 440.0, 160.0, 60.0, 60.0);
        pb.close();
        pb.finish().unwrap()
    };

    let path2 = {
        let mut pb = PathBuilder::new();
        pb.move_to(940.0, 60.0);
        pb.line_to(840.0, 940.0);
        pb.cubic_to(620.0, 840.0, 340.0, 800.0, 60.0, 800.0);
        pb.cubic_to(260.0, 460.0, 560.0, 160.0, 940.0, 60.0);
        pb.close();
        pb.finish().unwrap()
    };

    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill_path(
        &path1,
        &paint1,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    pixmap.fill_path(
        &path2,
        &paint2,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    pixmap
}

impl ExWlShellHandler for Window {
    type Ret = i32;
    fn return_value(&self) -> Option<Self::Ret> {
        Some(100)
    }
    fn on_init(
        &mut self,
        _state: &mut ExWlGlobalShellState,
        event: ExWlShellInitEvent,
    ) -> InitRequest {
        match event {
            // NOTE: this will send when init, you can request bind extra object from here
            ExWlShellInitEvent::Start => InitRequest::RequestBind,
            ExWlShellInitEvent::BindProvide(globals, qh) => {
                // NOTE: you can get implied wayland object from here
                let virtual_keyboard_manager = globals
                    .bind::<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                        qh,
                        1..=1,
                        (),
                    )
                    .unwrap();
                println!("{:?}", virtual_keyboard_manager);
                InitRequest::RequestCompositor
            }
            ExWlShellInitEvent::CompositorProvide(_compositor, _qh) => {
                // NOTE: this is an example to use the CompositorProvide,
                // but this is quite useless, because you can get the window_unit to set it directly
                // NOTE: you can set input region to limit area which gets input events
                // surface outside region becomes transparent for input events
                // To ignore all input events use region with (0,0) size
                // for x in state.get_unit_iter() {
                //     let region = _compositor.create_region(_qh, ());
                //     region.add(0, 0, 0, 0);
                //     x.get_wlsurface().set_input_region(Some(&region));
                // }
                InitRequest::None
            }
        }
    }

    fn on_normal_dispatch(&mut self, _context: NoIdEventContext<Self>) {}

    /// NOTE: you should know whether you have the buffer for this surface or not
    /// So no on created
    fn on_refresh(&mut self, context: RefreshEventContext<Self>) {
        let id = context.id();

        let mut resized = context.resized();
        let ex_wlshell_window = context.shell_state();

        let dpi::PhysicalSize { width, height } = ex_wlshell_window.physical_size();
        if !self.buffers.contains_key(&id) {
            self.buffers.insert(
                id,
                softbuffer::Surface::new(&self.context, ex_wlshell_window.gen_wrapper()).unwrap(),
            );
            resized = true;
        }
        let surface = self.buffers.get_mut(&id).unwrap();
        if resized {
            surface
                .configure(
                    NonZeroU32::new(width).unwrap(),
                    NonZeroU32::new(height).unwrap(),
                    softbuffer::AlphaMode::Premultiplied,
                )
                .unwrap();
        }
        let mut buffer = surface.next_buffer().unwrap();

        let pixmap = draw(width, height);
        for (x, y, pixel) in buffer.pixels_iter() {
            let sika_pixel = pixmap.pixel(x, y).unwrap();
            let b = sika_pixel.blue();
            let g = sika_pixel.green();
            let r = sika_pixel.red();
            let a = sika_pixel.alpha();

            *pixel = softbuffer::Pixel::new_bgra(b, g, r, a);
        }
        buffer.present().unwrap();
    }

    fn on_broadcast(&mut self, _context: NoIdEventContext<Self>, event: ExWlShellBroadcast) {
        if let ExWlShellBroadcast::OutputAdded(info) = event {
            println!("{info:?}");
        }
    }
    fn on_window_event(&mut self, mut context: HaveIdEventContext<Self>, event: ExWlShellEvent) {
        let id = context.id();
        let state = context.global_state_mut();
        match event {
            ExWlShellEvent::MouseEnter { pointer, .. } => {
                state.set_cursor(Cursor::Shape(CursorShape::Crosshair), &pointer)
            }
            ExWlShellEvent::MouseMotion {
                time,
                surface_x,
                surface_y,
                ..
            } => {
                println!("{time}, {surface_x}, {surface_y}");
            }
            ExWlShellEvent::OutputChanged(output) => {
                // NOTE: sent when surface enters another output, or its output info changes
                let info = output.as_ref().and_then(|o| state.get_output_info_of(o));
                println!("{info:?}");
            }
            ExWlShellEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                    state.exit();
                }
            }
            ExWlShellEvent::Closed => {
                self.buffers.remove(&id);
            }
            _ => {}
        }
    }
}

fn main() {
    let prebuild = ExWlEventLoopBuilder::new("Hello")
        .with_allscreens()
        .with_size(LayerSize::fill_width(400))
        .with_layer(Layer::Top)
        .with_margin(Margin {
            top: 20,
            right: 20,
            bottom: 100,
            left: 20,
        })
        .with_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right)
        .with_keyboard_interacivity(KeyboardInteractivity::Exclusive)
        .with_exclusive_zone(-1)
        .prebuild()
        .unwrap();
    let display = prebuild.display_wrapper();
    let context = softbuffer::Context::new(display).unwrap();
    let window = Window {
        context,
        buffers: HashMap::new(),
    };
    let ev = prebuild.attach(window).unwrap();

    let _ret = ev.run().unwrap();
    println!("{_ret:?}");
}
