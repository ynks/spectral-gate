use baseview::{
    Event, EventStatus, MouseButton, MouseEvent, ScrollDelta, WindowHandle, WindowOpenOptions,
    WindowScalePolicy,
};
use crossbeam::atomic::AtomicCell;
use keyboard_types::Modifiers;
use nih_plug::params::internals::ParamPtr;
use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use serde::{Deserialize, Serialize};
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{Key, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{
    ComponentHandle, LogicalPosition, Model, ModelRc, PhysicalSize, SharedString, VecModel,
};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod handles;
mod platform;
mod spectrum;

use crate::params::{
    curve_at, curve_point_pos, pos_to_freq, threshold_at, SgParams, CURVE_POINTS, CURVE_RANGE_DB,
};
use crate::shared::SpectrumFrame;
use handles::{surface_target, ParentHandle, SurfaceTarget};
use spectrum::{db_to_y, y_to_db, SpectrumRenderer, ViewParams, VIEW_HEIGHT, VIEW_WIDTH};

slint::include_modules!();

pub const DEFAULT_SIZE: (u32, u32) = (560, 556);

const CURVE_FIRST: usize = 16;

type Frames = Arc<Mutex<triple_buffer::Output<SpectrumFrame>>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct EditorState {
    #[serde(with = "nih_plug::params::persist::serialize_atomic_cell")]
    size: AtomicCell<(u32, u32)>,
    #[serde(skip)]
    open: AtomicBool,
}

impl EditorState {
    pub fn from_size(size: (u32, u32)) -> Arc<Self> {
        Arc::new(Self {
            size: AtomicCell::new(size),
            open: AtomicBool::new(false),
        })
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

impl<'a> PersistentField<'a, EditorState> for Arc<EditorState> {
    fn set(&self, new_value: EditorState) {
        self.size.store(new_value.size.load());
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&EditorState) -> R,
    {
        f(self)
    }
}

pub fn create(params: Arc<SgParams>, frames: Frames) -> Option<Box<dyn Editor>> {
    Some(Box::new(SgEditor {
        params,
        frames,
        #[cfg(target_os = "macos")]
        scale: AtomicCell::new(None),
        #[cfg(not(target_os = "macos"))]
        scale: AtomicCell::new(Some(1.0)),
    }))
}

struct SgEditor {
    params: Arc<SgParams>,
    frames: Frames,
    scale: AtomicCell<Option<f32>>,
}

impl Editor for SgEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        let (width, height) = DEFAULT_SIZE;
        let scale = self.scale.load();
        let params = self.params.clone();
        let frames = self.frames.clone();

        let window = baseview::Window::open_parented(
            &ParentHandle(parent),
            WindowOpenOptions {
                title: String::from("Spectral Gate"),
                size: baseview::Size::new(width as f64, height as f64),
                scale: scale
                    .map(|s| WindowScalePolicy::ScaleFactor(s as f64))
                    .unwrap_or(WindowScalePolicy::SystemScaleFactor),
            },
            move |window: &mut baseview::Window<'_>| {
                EditorWindow::new(window, context, params, frames, scale.unwrap_or(1.0))
            },
        );

        self.params.editor_state.open.store(true, Ordering::Release);
        Box::new(EditorHandle {
            state: self.params.editor_state.clone(),
            window,
        })
    }

    fn size(&self) -> (u32, u32) {
        DEFAULT_SIZE
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        if self.params.editor_state.is_open() {
            return false;
        }
        self.scale.store(Some(factor));
        true
    }

    // values get polled every frame instead
    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
    fn param_values_changed(&self) {}
}

struct EditorHandle {
    state: Arc<EditorState>,
    window: WindowHandle,
}

unsafe impl Send for EditorHandle {}

impl Drop for EditorHandle {
    fn drop(&mut self) {
        self.state.open.store(false, Ordering::Release);
        self.window.close();
    }
}

#[derive(Clone)]
struct Setter {
    ctx: Arc<dyn GuiContext>,
    ptrs: Rc<Vec<ParamPtr>>,
}

impl Setter {
    fn begin(&self, i: usize) {
        if let Some(&p) = self.ptrs.get(i) {
            unsafe { self.ctx.raw_begin_set_parameter(p) };
        }
    }

    fn set(&self, i: usize, value: f32) {
        if let Some(&p) = self.ptrs.get(i) {
            unsafe {
                self.ctx
                    .raw_set_parameter_normalized(p, value.clamp(0.0, 1.0))
            };
        }
    }

    fn set_plain(&self, i: usize, plain: f32) {
        if let Some(&p) = self.ptrs.get(i) {
            self.set(i, unsafe { p.preview_normalized(plain) });
        }
    }

    fn end(&self, i: usize) {
        if let Some(&p) = self.ptrs.get(i) {
            unsafe { self.ctx.raw_end_set_parameter(p) };
        }
    }

    fn reset(&self, i: usize) {
        if let Some(&p) = self.ptrs.get(i) {
            self.begin(i);
            self.set(i, unsafe { p.default_normalized_value() });
            self.end(i);
        }
    }
}

struct EditorWindow {
    params: Arc<SgParams>,
    frames: Frames,

    _context: softbuffer::Context<SurfaceTarget>,
    surface: softbuffer::Surface<SurfaceTarget, SurfaceTarget>,
    size: (u32, u32),
    scale: f32,

    window: Rc<MinimalSoftwareWindow>,
    ui: AppWindow,

    ptrs: Rc<Vec<ParamPtr>>,
    param_model: Rc<VecModel<ParamState>>,
    last_values: Vec<f32>,
    handle_model: Rc<VecModel<f32>>,

    renderer: SpectrumRenderer,
    last_view: Option<ViewParams>,
    dirty: bool,
    idle: u32,

    cursor: LogicalPosition,
    modifiers: Modifiers,
}

impl EditorWindow {
    fn new(
        bv_window: &mut baseview::Window<'_>,
        ctx: Arc<dyn GuiContext>,
        params: Arc<SgParams>,
        frames: Frames,
        scale: f32,
    ) -> Self {
        let size = (
            (DEFAULT_SIZE.0 as f32 * scale).round() as u32,
            (DEFAULT_SIZE.1 as f32 * scale).round() as u32,
        );

        let target = surface_target(bv_window);
        let context = softbuffer::Context::new(target.clone()).expect("softbuffer context");
        let mut surface = softbuffer::Surface::new(&context, target).expect("softbuffer surface");
        surface
            .resize(
                NonZeroU32::new(size.0.max(1)).unwrap(),
                NonZeroU32::new(size.1.max(1)).unwrap(),
            )
            .expect("softbuffer resize");

        let window = platform::next_window();
        let ui = AppWindow::new().expect("slint ui");
        window.dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: scale,
        });
        window.set_size(PhysicalSize::new(size.0, size.1));
        ui.show().expect("slint show");

        let ptrs = Rc::new(params.ui_order());
        let param_model = Rc::new(VecModel::from(vec![ParamState::default(); ptrs.len()]));
        let handle_model = Rc::new(VecModel::from(vec![0.5f32; CURVE_POINTS]));

        let bridge = ui.global::<Bridge>();
        bridge.set_params(ModelRc::from(param_model.clone()));
        bridge.set_handle_y(ModelRc::from(handle_model.clone()));
        bind_callbacks(
            &bridge,
            Setter {
                ctx,
                ptrs: ptrs.clone(),
            },
            params.clone(),
        );

        let mut this = Self {
            params,
            frames,
            _context: context,
            surface,
            size,
            scale,
            window,
            ui,
            last_values: vec![f32::NAN; ptrs.len()],
            ptrs,
            param_model,
            handle_model,
            renderer: SpectrumRenderer::new(),
            last_view: None,
            dirty: true,
            idle: 0,
            cursor: LogicalPosition::new(0.0, 0.0),
            modifiers: Modifiers::empty(),
        };
        this.sync_params();
        this
    }

    fn sync_params(&mut self) {
        for (i, &ptr) in self.ptrs.iter().enumerate() {
            let value = unsafe { ptr.modulated_normalized_value() };
            if value == self.last_values[i] {
                continue;
            }
            self.last_values[i] = value;
            let state = unsafe {
                ParamState {
                    value,
                    plain: ptr.modulated_plain_value(),
                    text: SharedString::from(ptr.normalized_value_to_string(value, true)),
                }
            };
            self.param_model.set_row_data(i, state);
        }
    }

    fn view_params(&self) -> ViewParams {
        let p = &self.params;
        ViewParams {
            threshold_db: p.threshold.value(),
            tilt: p.tilt.value(),
            curve: p.curve_offsets(),
            low_hz: p.low_freq.value(),
            high_hz: p.high_freq.value(),
            fft_size: p.fft_size.value().samples(),
        }
    }

    fn update_spectrum(&mut self) {
        if let Ok(mut frames) = self.frames.lock() {
            if frames.update() {
                self.renderer.feed(frames.output_buffer());
                self.dirty = true;
            }
        }

        let view = self.view_params();
        if self.last_view != Some(view) {
            for i in 0..CURVE_POINTS {
                let freq = pos_to_freq(curve_point_pos(i));
                let y = db_to_y(threshold_at(
                    view.threshold_db,
                    view.tilt,
                    &view.curve,
                    freq,
                ));
                if self.handle_model.row_data(i) != Some(y) {
                    self.handle_model.set_row_data(i, y);
                }
            }
            self.last_view = Some(view);
            self.dirty = true;
        }

        if self.dirty || self.renderer.animating(&view) {
            self.dirty = false;
            let w = (VIEW_WIDTH * self.scale).round() as usize;
            let h = (VIEW_HEIGHT * self.scale).round() as usize;
            let img = self.renderer.render(w, h, &view);
            self.ui
                .global::<Bridge>()
                .set_spectrum(slint::Image::from_rgb8(img));
        }
    }

    // slint only knows about modifiers through key events
    fn sync_modifiers(&mut self, modifiers: Modifiers) {
        if modifiers == self.modifiers {
            return;
        }
        for (flag, key) in [
            (Modifiers::SHIFT, Key::Shift),
            (Modifiers::CONTROL, Key::Control),
            (Modifiers::ALT, Key::Alt),
            (Modifiers::META, Key::Meta),
        ] {
            let down = modifiers.contains(flag);
            if self.modifiers.contains(flag) != down {
                let text = SharedString::from(key);
                self.window.dispatch_event(if down {
                    WindowEvent::KeyPressed { text }
                } else {
                    WindowEvent::KeyReleased { text }
                });
            }
        }
        self.modifiers = modifiers;
    }
}

fn bind_callbacks(bridge: &Bridge, setter: Setter, params: Arc<SgParams>) {
    let s = setter.clone();
    bridge.on_begin(move |i| s.begin(i as usize));
    let s = setter.clone();
    bridge.on_set(move |i, v| s.set(i as usize, v));
    let s = setter.clone();
    bridge.on_end(move |i| s.end(i as usize));
    let s = setter.clone();
    bridge.on_reset(move |i| s.reset(i as usize));
    let s = setter.clone();
    bridge.on_set_plain_in_gesture(move |i, v| s.set_plain(i as usize, v));

    let s = setter.clone();
    let p = params.clone();
    bridge.on_drag_handle(move |point, y| {
        let point = (point as usize).min(CURVE_POINTS - 1);
        let freq = pos_to_freq(curve_point_pos(point));
        let offset = y_to_db(y) - p.threshold.value() - p.tilt.value() * (freq / 1000.0).log2();
        s.set_plain(
            CURVE_FIRST + point,
            offset.clamp(-CURVE_RANGE_DB, CURVE_RANGE_DB),
        );
    });

    let s = setter.clone();
    let p = params;
    bridge.on_drag_threshold(move |x, y| {
        let freq = pos_to_freq(x);
        let base =
            y_to_db(y) - p.tilt.value() * (freq / 1000.0).log2() - curve_at(&p.curve_offsets(), x);
        s.set_plain(0, base);
    });

    let s = setter;
    bridge.on_flatten_curve(move || {
        for i in 0..CURVE_POINTS {
            s.reset(CURVE_FIRST + i);
        }
    });
}

fn map_button(button: MouseButton) -> PointerEventButton {
    match button {
        MouseButton::Left => PointerEventButton::Left,
        MouseButton::Right => PointerEventButton::Right,
        MouseButton::Middle => PointerEventButton::Middle,
        _ => PointerEventButton::Other,
    }
}

impl baseview::WindowHandler for EditorWindow {
    fn on_frame(&mut self, _window: &mut baseview::Window) {
        slint::platform::update_timers_and_animations();
        self.sync_params();
        self.update_spectrum();

        // redraw every now and then in case the window got covered
        self.idle += 1;
        if self.idle >= 30 {
            self.window.request_redraw();
        }

        let (w, h) = self.size;
        let surface = &mut self.surface;
        let drawn = self.window.draw_if_needed(|renderer| {
            if let Ok(mut buffer) = surface.buffer_mut() {
                if buffer.len() == (w * h) as usize {
                    renderer.render(platform::as_pixels(&mut buffer), w as usize);
                    let _ = buffer.present();
                }
            }
        });
        if drawn {
            self.idle = 0;
        }
    }

    fn on_event(&mut self, _window: &mut baseview::Window, event: Event) -> EventStatus {
        match event {
            Event::Mouse(event) => {
                let event = match event {
                    MouseEvent::CursorMoved {
                        position,
                        modifiers,
                    } => {
                        self.sync_modifiers(modifiers);
                        self.cursor = LogicalPosition::new(position.x as f32, position.y as f32);
                        WindowEvent::PointerMoved {
                            position: self.cursor,
                        }
                    }
                    MouseEvent::ButtonPressed { button, modifiers } => {
                        self.sync_modifiers(modifiers);
                        WindowEvent::PointerPressed {
                            position: self.cursor,
                            button: map_button(button),
                        }
                    }
                    MouseEvent::ButtonReleased { button, modifiers } => {
                        self.sync_modifiers(modifiers);
                        WindowEvent::PointerReleased {
                            position: self.cursor,
                            button: map_button(button),
                        }
                    }
                    MouseEvent::WheelScrolled { delta, modifiers } => {
                        self.sync_modifiers(modifiers);
                        let (delta_x, delta_y) = match delta {
                            ScrollDelta::Lines { x, y } => (x * 40.0, y * 40.0),
                            ScrollDelta::Pixels { x, y } => (x, y),
                        };
                        WindowEvent::PointerScrolled {
                            position: self.cursor,
                            delta_x,
                            delta_y,
                        }
                    }
                    MouseEvent::CursorLeft => WindowEvent::PointerExited,
                    _ => return EventStatus::Ignored,
                };
                self.window.dispatch_event(event);
                EventStatus::Captured
            }
            Event::Window(baseview::WindowEvent::Resized(info)) => {
                let size = info.physical_size();
                self.scale = info.scale() as f32;
                self.size = (size.width.max(1), size.height.max(1));
                let _ = self.surface.resize(
                    NonZeroU32::new(self.size.0).unwrap(),
                    NonZeroU32::new(self.size.1).unwrap(),
                );
                self.window.dispatch_event(WindowEvent::ScaleFactorChanged {
                    scale_factor: self.scale,
                });
                self.window
                    .set_size(PhysicalSize::new(self.size.0, self.size.1));
                self.dirty = true;
                self.window.request_redraw();
                EventStatus::Captured
            }
            // let the host keep its shortcuts
            _ => EventStatus::Ignored,
        }
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        let _ = self.ui.hide();
        let _ = self.window.window().hide();
    }
}
