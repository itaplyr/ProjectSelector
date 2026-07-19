mod app;
mod projects;

mod egl_raw {
    #![allow(dead_code)]
    use std::ffi::c_void;
    use std::os::raw::c_int;

    pub type EGLDisplay = *mut c_void;
    pub type EGLConfig = *mut c_void;
    pub type EGLContext = *mut c_void;
    pub type EGLSurface = *mut c_void;
    pub type EGLint = c_int;
    pub type EGLBoolean = u32;

    pub const EGL_TRUE: EGLBoolean = 1;
    pub const EGL_NONE: EGLint = 0x3038;
    pub const EGL_RED_SIZE: EGLint = 0x3024;
    pub const EGL_GREEN_SIZE: EGLint = 0x3023;
    pub const EGL_BLUE_SIZE: EGLint = 0x3022;
    pub const EGL_ALPHA_SIZE: EGLint = 0x3021;
    pub const EGL_SURFACE_TYPE: EGLint = 0x3033;
    pub const EGL_WINDOW_BIT: EGLint = 0x0004;
    pub const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
    pub const EGL_OPENGL_ES3_BIT: EGLint = 0x0040;
    pub const EGL_CONTEXT_MAJOR_VERSION: EGLint = 0x3098;
    pub const EGL_CONTEXT_MINOR_VERSION: EGLint = 0x30fb;
    pub const EGL_OPENGL_ES_API: EGLint = 0x30a0;

    #[link(name = "EGL")]
    extern "C" {
        pub fn eglGetDisplay(display_id: EGLDisplay) -> EGLDisplay;
        pub fn eglInitialize(display: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
        pub fn eglBindAPI(api: EGLint) -> EGLBoolean;
        pub fn eglChooseConfig(
            display: EGLDisplay, attrib_list: *const EGLint,
            configs: *mut EGLConfig, config_size: EGLint, num_config: *mut EGLint,
        ) -> EGLBoolean;
        pub fn eglCreateContext(
            display: EGLDisplay, config: EGLConfig,
            share_context: EGLContext, attrib_list: *const EGLint,
        ) -> EGLContext;
        pub fn eglCreateWindowSurface(
            display: EGLDisplay, config: EGLConfig,
            win: *mut c_void, attrib_list: *const EGLint,
        ) -> EGLSurface;
        pub fn eglMakeCurrent(
            display: EGLDisplay, draw: EGLSurface,
            read: EGLSurface, context: EGLContext,
        ) -> EGLBoolean;
        pub fn eglSwapBuffers(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
        pub fn eglGetProcAddress(name: *const std::os::raw::c_char) -> *mut c_void;
        pub fn eglDestroySurface(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    }
}

use std::ffi::c_void;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{
            KeyEvent, KeyboardHandler, Keysym, Modifiers,
        },
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

const WIDTH: u32 = 382;
const HEIGHT: u32 = 402;
const NAMESPACE: &str = "project-selector";

fn keysym_to_egui_key(keysym: Keysym) -> Option<egui::Key> {
    use egui::Key;
    let s = keysym.raw();
    match s {
        0xff1b => Some(Key::Escape),
        0xff0d => Some(Key::Enter),
        0xff09 => Some(Key::Tab),
        0xff08 => Some(Key::Backspace),
        0xffff => Some(Key::Delete),
        0xff52 => Some(Key::ArrowUp),
        0xff54 => Some(Key::ArrowDown),
        0xff51 => Some(Key::ArrowLeft),
        0xff53 => Some(Key::ArrowRight),
        0xff50 => Some(Key::Home),
        0xff57 => Some(Key::End),
        0x0061 => Some(Key::A), 0x0062 => Some(Key::B), 0x0063 => Some(Key::C),
        0x0064 => Some(Key::D), 0x0065 => Some(Key::E), 0x0066 => Some(Key::F),
        0x0067 => Some(Key::G), 0x0068 => Some(Key::H), 0x0069 => Some(Key::I),
        0x006a => Some(Key::J), 0x006b => Some(Key::K), 0x006c => Some(Key::L),
        0x006d => Some(Key::M), 0x006e => Some(Key::N), 0x006f => Some(Key::O),
        0x0070 => Some(Key::P), 0x0071 => Some(Key::Q), 0x0072 => Some(Key::R),
        0x0073 => Some(Key::S), 0x0074 => Some(Key::T), 0x0075 => Some(Key::U),
        0x0076 => Some(Key::V), 0x0077 => Some(Key::W), 0x0078 => Some(Key::X),
        0x0079 => Some(Key::Y), 0x007a => Some(Key::Z),
        0x0030 => Some(Key::Num0), 0x0031 => Some(Key::Num1), 0x0032 => Some(Key::Num2),
        0x0033 => Some(Key::Num3), 0x0034 => Some(Key::Num4), 0x0035 => Some(Key::Num5),
        0x0036 => Some(Key::Num6), 0x0037 => Some(Key::Num7), 0x0038 => Some(Key::Num8),
        0x0039 => Some(Key::Num9),
        _ => None,
    }
}

struct AppState {
    registry_state: RegistryState,
    _compositor_state: CompositorState,
    _layer_state: LayerShell,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,

    layer: LayerSurface,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    egl_display: egl_raw::EGLDisplay,
    egl_config: egl_raw::EGLConfig,
    egl_context: egl_raw::EGLContext,
    egl_surface: Option<egl_raw::EGLSurface>,
    egl_window: Option<wayland_egl::WlEglSurface>,

    egui_ctx: egui::Context,
    painter: Option<egui_glow::Painter>,

    app: app::ProjectApp,

    width: i32,
    height: i32,
    running: bool,
    configured: bool,
    first_render: bool,

    pending_events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
}

delegate_registry!(AppState);
delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_layer!(AppState);
delegate_seat!(AppState);
delegate_keyboard!(AppState);
delegate_pointer!(AppState);
delegate_shm!(AppState);

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _new_factor: i32,
    ) {}
    fn transform_changed(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _new_transform: wl_output::Transform,
    ) {}
    fn frame(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _time: u32,
    ) {}
    fn surface_enter(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _output: &wl_output::WlOutput,
    ) {}
    fn surface_leave(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _output: &wl_output::WlOutput,
    ) {}
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _layer: &LayerSurface, configure: LayerSurfaceConfigure, _serial: u32,
    ) {
        let (w, h) = configure.new_size;
        self.width = if w != 0 { w as i32 } else { WIDTH as i32 };
        self.height = if h != 0 { h as i32 } else { HEIGHT as i32 };

        if !self.configured {
            self.configured = true;
            self.create_egl_surface();
            if self.egl_surface.is_some() {
                self.first_render = true;
            }
        }
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self, _conn: &Connection, qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat, capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self.seat_state
                .get_keyboard(qh, &seat, None)
                .expect("failed to create keyboard");
            self.keyboard = Some(keyboard);
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self.seat_state
                .get_pointer(qh, &seat)
                .expect("failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self, _conn: &Connection, _: &QueueHandle<Self>,
        _: wl_seat::WlSeat, capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(kb) = self.keyboard.take() { kb.release(); }
        }
        if capability == Capability::Pointer {
            if let Some(ptr) = self.pointer.take() { ptr.release(); }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _surface: &wl_surface::WlSurface,
        _: u32, _: &[u32], _: &[Keysym],
    ) {}

    fn leave(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _surface: &wl_surface::WlSurface, _: u32,
    ) {
        self.running = false;
    }

    fn press_key(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _serial: u32, event: KeyEvent,
    ) {
        if let Some(key) = keysym_to_egui_key(event.keysym) {
            self.pending_events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
        if let Some(text) = event.utf8 {
            let is_printable = text.chars().all(|c| !c.is_control());
            if is_printable && !text.is_empty() {
                self.pending_events.push(egui::Event::Text(text));
            }
        }
    }

    fn release_key(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _serial: u32, event: KeyEvent,
    ) {
        if let Some(key) = keysym_to_egui_key(event.keysym) {
            self.pending_events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
    }

    fn repeat_key(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _serial: u32, event: KeyEvent,
    ) {
        if let Some(key) = keysym_to_egui_key(event.keysym) {
            self.pending_events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: self.modifiers,
            });
        }
    }

    fn update_modifiers(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard, _serial: u32,
        modifiers: Modifiers, _raw: smithay_client_toolkit::seat::keyboard::RawModifiers, _layout: u32,
    ) {
        self.modifiers = egui::Modifiers {
            alt: modifiers.alt,
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            command: modifiers.logo,
            ..Default::default()
        };
    }
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer, events: &[PointerEvent],
    ) {
        for event in events {
            if event.surface != *self.layer.wl_surface() {
                continue;
            }
            let pos = egui::pos2(event.position.0 as f32, event.position.1 as f32);
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.pending_events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Leave { .. } => {
                    self.pending_events.push(egui::Event::PointerGone);
                    self.running = false;
                }
                PointerEventKind::Motion { .. } => {
                    self.pending_events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Press { button, .. } => {
                    let btn = match button {
                        0x110 => egui::PointerButton::Primary,
                        0x111 => egui::PointerButton::Secondary,
                        0x112 => egui::PointerButton::Middle,
                        _ => egui::PointerButton::Primary,
                    };
                    self.pending_events.push(egui::Event::PointerButton {
                        pos, button: btn, pressed: true, modifiers: self.modifiers,
                    });
                }
                PointerEventKind::Release { button, .. } => {
                    let btn = match button {
                        0x110 => egui::PointerButton::Primary,
                        0x111 => egui::PointerButton::Secondary,
                        0x112 => egui::PointerButton::Middle,
                        _ => egui::PointerButton::Primary,
                    };
                    self.pending_events.push(egui::Event::PointerButton {
                        pos, button: btn, pressed: false, modifiers: self.modifiers,
                    });
                }
                PointerEventKind::Axis { vertical, .. } => {
                    self.pending_events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0.0, vertical.discrete as f32),
                        phase: egui::TouchPhase::Move,
                        modifiers: self.modifiers,
                    });
                }
            }
        }
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}

impl AppState {
    fn init_egl(&mut self, conn: &Connection) {
        let raw_display = conn.backend().display_ptr() as *mut c_void;

        unsafe {
            let display = egl_raw::eglGetDisplay(raw_display);
            if display.is_null() {
                eprintln!("[egl] failed to get display");
                return;
            }
            let mut major = 0i32;
            let mut minor = 0i32;
            if egl_raw::eglInitialize(display, &mut major, &mut minor) == 0 {
                eprintln!("[egl] failed to initialize");
                return;
            }
            eprintln!("[egl] initialized {}.{}", major, minor);

            egl_raw::eglBindAPI(egl_raw::EGL_OPENGL_ES_API);

            let config_attrs = [
                egl_raw::EGL_RED_SIZE, 8,
                egl_raw::EGL_GREEN_SIZE, 8,
                egl_raw::EGL_BLUE_SIZE, 8,
                egl_raw::EGL_ALPHA_SIZE, 8,
                egl_raw::EGL_SURFACE_TYPE, egl_raw::EGL_WINDOW_BIT,
                egl_raw::EGL_RENDERABLE_TYPE, egl_raw::EGL_OPENGL_ES3_BIT,
                egl_raw::EGL_NONE,
            ];
            let mut num_configs = 0i32;
            let mut config = std::ptr::null_mut();
            if egl_raw::eglChooseConfig(
                display, config_attrs.as_ptr(),
                &mut config, 1, &mut num_configs,
            ) == 0 || num_configs == 0 {
                eprintln!("[egl] failed to choose config");
                return;
            }

            let ctx_attrs = [
                egl_raw::EGL_CONTEXT_MAJOR_VERSION, 3,
                egl_raw::EGL_CONTEXT_MINOR_VERSION, 0,
                egl_raw::EGL_NONE,
            ];
            let context = egl_raw::eglCreateContext(
                display, config, std::ptr::null_mut(), ctx_attrs.as_ptr(),
            );
            if context.is_null() {
                eprintln!("[egl] failed to create context");
                return;
            }

            self.egl_display = display;
            self.egl_config = config;
            self.egl_context = context;
        }
    }

    fn create_egl_surface(&mut self) {
        if self.egl_display.is_null() {
            return;
        }

        let egl_window = wayland_egl::WlEglSurface::new(
            self.layer.wl_surface().id(), self.width, self.height,
        );
        let egl_window = match egl_window {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[egl] failed to create wayland egl window: {:?}", e);
                return;
            }
        };

        let native_window = egl_window.ptr() as *mut c_void;

        let surface = unsafe {
            egl_raw::eglCreateWindowSurface(
                self.egl_display, self.egl_config,
                native_window, std::ptr::null(),
            )
        };
        if surface.is_null() {
            eprintln!("[egl] failed to create window surface");
            return;
        }

        unsafe {
            egl_raw::eglMakeCurrent(
                self.egl_display, surface, surface, self.egl_context,
            );
        }

        let glow_ctx = unsafe {
            glow::Context::from_loader_function_cstr(|name| {
                egl_raw::eglGetProcAddress(name.as_ptr()) as *const _
            })
        };

        let painter = egui_glow::Painter::new(
            std::sync::Arc::new(glow_ctx),
            "",
            None,
            false,
        )
        .expect("failed to create egui painter");

        self.egl_window = Some(egl_window);
        self.egl_surface = Some(surface);
        self.painter = Some(painter);

        eprintln!("[egl] surface created {}x{}", self.width, self.height);
    }

    fn render(&mut self) {
        let painter = match self.painter.as_mut() {
            Some(p) => p,
            None => return,
        };

        let events = std::mem::take(&mut self.pending_events);

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(self.width as f32, self.height as f32),
            )),
            events,
            ..Default::default()
        };

        self.egui_ctx.begin_pass(raw_input);
        self.app.update(&self.egui_ctx);
        let full_output = self.egui_ctx.end_pass();

        if self.app.should_close {
            self.running = false;
            return;
        }

        let clipped = self.egui_ctx.tessellate(full_output.shapes, 1.0);
        painter.paint_and_update_textures(
            [self.width as u32, self.height as u32],
            1.0,
            &clipped,
            &full_output.textures_delta,
        );

        unsafe {
            if let Some(surface) = self.egl_surface {
                egl_raw::eglSwapBuffers(self.egl_display, surface);
            }
        }

        self.first_render = false;
    }
}

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().expect("failed to connect to Wayland");
    let (globals, mut event_queue) = registry_queue_init::<AppState>(&conn)
        .expect("failed to init registry");
    let qh = event_queue.handle();

    let compositor_state = CompositorState::bind(&globals, &qh)
        .expect("wl_compositor not available");
    let layer_state = LayerShell::bind(&globals, &qh)
        .expect("layer shell not available");
    let shm = Shm::bind(&globals, &qh)
        .expect("wl_shm not available");

    let surface = compositor_state.create_surface(&qh);
    let layer = layer_state.create_layer_surface(
        &qh, surface, Layer::Top, Some(NAMESPACE), None,
    );
    layer.set_anchor(Anchor::empty());
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_size(WIDTH, HEIGHT);
    layer.set_exclusive_zone(-1);
    layer.commit();

    let mut state = AppState {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        _compositor_state: compositor_state,
        _layer_state: layer_state,
        shm,

        layer,
        keyboard: None,
        pointer: None,

        egl_display: std::ptr::null_mut(),
        egl_config: std::ptr::null_mut(),
        egl_context: std::ptr::null_mut(),
        egl_surface: None,
        egl_window: None,

        egui_ctx: egui::Context::default(),
        painter: None,

        app: app::ProjectApp::default(),

        width: WIDTH as i32,
        height: HEIGHT as i32,
        running: true,
        configured: false,
        first_render: false,

        pending_events: Vec::new(),
        modifiers: egui::Modifiers::default(),
    };

    state.init_egl(&conn);

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        if !state.running {
            break;
        }

        if state.configured && (state.first_render || !state.pending_events.is_empty()) {
            state.render();
        }
    }

    if let Some(painter) = &mut state.painter {
        painter.destroy();
    }
    eprintln!("[main] exiting");
}
