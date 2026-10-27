use std::collections::{HashMap, VecDeque};
use std::num::NonZeroU32;
use std::sync::Arc;

use exwlshellev::keyboard::{KeyCode, PhysicalKey};
use exwlshellev::reexport::*;
use exwlshellev::*;
use tiny_skia::*;

struct Window {
    context: softbuffer::Context<DisplayWrapper>,
    buffers: HashMap<id::Id, softbuffer::Surface<DisplayWrapper, WindowWrapper>>,
}

#[derive(Debug, Clone, Copy)]
struct Position<T = f32> {
    x: T,
    y: T,
}

// I want to make a basic drawing surface, so I need
// brush, cursor, rectangle
// Then I need undo and redo
// So I need to maintain two machine states

/// Line, record the line
#[derive(Debug, Clone, Copy)]
struct Line {
    start: Position,
    end: Position,
    color: Color,
    line: f32,
}

#[derive(Debug, Clone, Copy)]
struct Circle {
    center: Position,
    radius: f32,
    color: Color,
}

trait Element {
    fn draw(&self, _pixmap: &mut Pixmap) {}
}

struct StateMachine {
    current: VecDeque<Box<dyn Element>>,
    restore: VecDeque<Box<dyn Element>>,
}

impl StateMachine {
    fn op_line(&mut self, operator: Line) {
        let redo: Box<dyn Element> = Box::new(operator);
        self.current.push_back(redo);
        self.restore.clear();
    }
    fn op_circle(&mut self, operator: Circle) {
        let redo: Box<dyn Element> = Box::new(operator);
        self.current.push_back(redo);
        self.restore.clear();
    }
    fn undo(&mut self) {
        let Some(redo) = self.current.pop_back() else {
            return;
        };
        self.restore.push_back(redo);
    }
    fn redo(&mut self) {
        let Some(undo) = self.restore.pop_back() else {
            return;
        };
        self.current.push_back(undo);
    }
    fn redraw(&self, pixmap: &mut Pixmap) {
        for curr in &self.current {
            curr.draw(pixmap);
        }
    }
}

impl Element for Line {
    fn draw(&self, pixmap: &mut Pixmap) {
        let mut paint = Paint::default();
        paint.set_color(self.color);
        paint.anti_alias = true;
        let path = {
            let mut pb = PathBuilder::new();
            pb.move_to(self.start.x, self.start.y);
            pb.line_to(self.end.x, self.end.y);
            pb.close();
            pb.finish().unwrap()
        };

        pixmap.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: self.line,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
}

impl Element for Circle {
    fn draw(&self, pixmap: &mut Pixmap) {
        let mut paint = Paint::default();
        paint.set_color(self.color);
        paint.anti_alias = true;
        let path = {
            let mut pb = PathBuilder::new();
            pb.push_circle(self.center.x, self.center.y, self.radius);
            pb.close();
            pb.finish().unwrap()
        };

        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}
// ViewPort is use to clip a part of the Pixmap area, and just render this part
// It should be the same of the wayland surface
// but it allows width and height to be minus, that means the direction
#[derive(Debug, Clone, Copy)]
struct ViewPort {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
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
            let Some(sika_pixel) = pixmap.pixel(x, y) else {
                continue;
            };
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
