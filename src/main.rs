#![allow(clippy::approx_constant)]

use std::sync::{Arc, Mutex, mpsc};

use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

mod audio;
mod constants;
mod graphics;
mod shaders;

struct State {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    pipeline: crate::graphics::Pipeline,

    audio: Option<Audio>,
}

struct Audio {
    output_stream: rodio::OutputStream,
    sink: rodio::Sink,
    // TODO: better naming
    tx: mpsc::SyncSender<()>,
    bins: Arc<Mutex<Vec<f32>>>,
}

impl State {
    async fn new(flags: &flags::Main, window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let size = window.inner_size();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        let pipeline = graphics::Pipeline::new(&device, &queue, size, surface_format);

        let mut state = State {
            window,
            device,
            queue,
            size,
            surface,
            surface_format,
            pipeline,
            audio: None,
        };

        // Configure surface for the first time
        state.configure_surface();

        // TODO: some way to stop this sound
        if let Some(file) = &flags.music {
            let output_stream = rodio::OutputStreamBuilder::open_default_stream()
                .expect("could not open default stream");
            let mixer = output_stream.mixer();
            let sink = rodio::Sink::connect_new(mixer);

            let file = std::fs::File::open(file).expect("could not open music file");
            let source = rodio::Decoder::try_from(file).expect("could not decode music file");
            let (collector, source) = audio::collector::Collector::new(source);
            sink.append(source);

            let (tx, bins, worker) = audio::worker::Worker::new(collector);
            std::thread::spawn(move || worker.work());

            state.audio = Some(Audio {
                output_stream,
                sink,
                tx,
                bins,
            });
        }

        state
    }

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn configure_surface(&mut self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            // Request compatibility with the sRGB-format texture view we‘re going to create later.
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 1,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &surface_config);
        self.pipeline.resize(&self.queue, self.size);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;

        // reconfigure the surface
        self.configure_surface();
    }

    fn render(&mut self) {
        // Create texture view
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");

        self.pipeline.render(
            &self.device,
            &self.queue,
            &surface_texture.texture,
            self.surface_format,
        );

        self.window.pre_present_notify();
        surface_texture.present();
    }
}

struct App {
    flags: flags::Main,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create window object
        let window_attributes = Window::default_attributes().with_title("physarum-36p-rs");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let state = pollster::block_on(State::new(&self.flags, window.clone()));
        self.state = Some(state);

        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.render();
                // Emits a new redraw requested event.
                state.get_window().request_redraw();

                if let Some(audio) = &state.audio {
                    audio::worker::submit_work(&audio.tx);
                    println!("{:?}", audio.bins.lock().unwrap());
                }
            }
            WindowEvent::Resized(size) => {
                // Reconfigures the size of the surface. We do not re-render
                // here as this event is always followed up by redraw request.
                state.resize(size);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::F11),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                // Toggle fullscreen
                let window = state.get_window();
                if window.fullscreen().is_some() {
                    window.set_fullscreen(None);
                } else {
                    window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                state.pipeline.handle_keypress(&state.queue, key);
            }
            _ => (),
        }
    }
}

mod flags {
    use std::path::PathBuf;

    xflags::xflags! {
        cmd main {
            optional --music file: PathBuf
        }
    }
}

fn main() {
    // wgpu uses `log` for all of our logging, so we initialize a logger with the `env_logger` crate.
    //
    // To change the log level, set the `RUST_LOG` environment variable. See the `env_logger`
    // documentation for more information.
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

    // When the current loop iteration finishes, suspend the thread until
    // another event arrives. Helps keeping CPU utilization low if nothing
    // is happening, which is preferred if the application might be idling in
    // the background.
    // event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App {
        flags: flags::Main::from_env_or_exit(),
        state: None,
    };
    event_loop.run_app(&mut app).unwrap();
}
