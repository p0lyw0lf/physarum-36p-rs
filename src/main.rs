#![allow(clippy::approx_constant)]

use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use rodio::{DeviceTrait, Source, cpal::traits::HostTrait};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

use crate::audio::NUM_BINS;

mod audio;
mod constants;
mod fs;
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
    // We're required to keep ownership of this so that the audio continues playing
    _output_stream: rodio::OutputStream,
    sink: rodio::Sink,
    total_duration: Duration,
    // TODO: better naming
    tx: mpsc::SyncSender<()>,
    bins: Arc<Mutex<Vec<f32>>>,
    last_bins: [f32; NUM_BINS],
}

/// Data that gets rendered on the screen every frame, if playing audio
struct AudioDisplay {
    bins: [f32; NUM_BINS],
    position: Duration,
    total_duration: Duration,
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

        let settings_filename = flags.settings.clone().unwrap_or("settings.json".into());
        let mut pipeline = graphics::Pipeline::new(&device, &queue, size, surface_format);
        pipeline.read_settings_file(&queue, settings_filename);
        let pipeline = pipeline;

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

        if let Some(file) = &flags.music {
            /// Returns a PulseAudio device, if there is one.
            /// cpal only supports ALSA on Linux, but fortunately that has a PulseAudio backend
            /// which seems to be the thing we actually use on KDE for routing audio stuff.
            fn find_pulse_device() -> Option<rodio::cpal::Device> {
                #[cfg(target_os = "linux")]
                for device in rodio::cpal::host_from_id(rodio::cpal::HostId::Alsa)
                    .expect("could not open host")
                    .output_devices()
                    .expect("could not enumerate output devices")
                {
                    let name = device.name().expect("could not read device name");
                    if name == "pulse" {
                        return Some(device);
                    }
                }

                None
            }
            let output_stream = match find_pulse_device() {
                Some(device) => rodio::OutputStreamBuilder::from_device(device),
                None => rodio::OutputStreamBuilder::from_default_device(),
            }
            .expect("could not build output stream from device")
            .open_stream()
            .expect("could not open output stream");
            let mixer = output_stream.mixer();
            // TODO: some way to pause/otherwise control this sink with the keyboard
            let sink = rodio::Sink::connect_new(mixer);

            let file = std::fs::File::open(file).expect("could not open music file");
            let source = rodio::Decoder::try_from(file).expect("could not decode music file");
            let total_duration = source
                .total_duration()
                .expect("could not get source duration");
            let (collector, source) = audio::collector::Collector::new(source);
            sink.append(source);

            let (tx, bins, worker) = audio::worker::Worker::new(collector);
            std::thread::spawn(move || worker.work());

            state.audio = Some(Audio {
                _output_stream: output_stream,
                sink,
                total_duration,
                tx,
                bins,
                last_bins: [0.0; NUM_BINS],
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

    fn render(&mut self, data: Option<&AudioDisplay>) {
        // Create texture view
        if let Ok(surface_texture) = self.surface.get_current_texture() {
            self.pipeline.render(
                &self.device,
                &self.queue,
                &surface_texture.texture,
                self.surface_format,
                data,
            );

            self.window.pre_present_notify();
            surface_texture.present();
        } else {
            // Surface texture creation failed for whatever reason; on Linux, this usually means
            // that the window was drawn over by something else.
        }
    }
}

impl State {
    fn handle_music_key(&mut self, key: KeyCode, repeat: bool) -> bool {
        let audio = match self.audio.as_mut() {
            Some(audio) => audio,
            None => return false,
        };
        match key {
            KeyCode::F2 => {
                let pos = audio.sink.get_pos();
                let next_pos = pos.saturating_sub(Duration::from_secs(10));
                match audio.sink.try_seek(next_pos) {
                    Ok(()) => {}
                    Err(err) => eprintln!("Error seeking backwards: {err}"),
                };
                true
            }
            KeyCode::F3 if !repeat => {
                if audio.sink.is_paused() {
                    audio.sink.play();
                } else {
                    audio.sink.pause();
                }
                self.pipeline.set_playing(!audio.sink.is_paused());
                true
            }
            KeyCode::F4 => {
                let pos = audio.sink.get_pos();
                let next_pos = pos.saturating_add(Duration::from_secs(10));
                match audio.sink.try_seek(next_pos) {
                    Ok(()) => {}
                    Err(err) => eprintln!("Error seeking forwards: {err}"),
                };
                true
            }
            _ => false,
        }
    }
}

struct App {
    flags: flags::Main,
    close_requested: bool,
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

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                self.close_requested = true;
            }
            WindowEvent::RedrawRequested => {
                let data = state.audio.as_ref().map(|audio| AudioDisplay {
                    bins: audio.last_bins,
                    position: audio.sink.get_pos(),
                    total_duration: audio.total_duration,
                });
                state.render(data.as_ref());

                // Request another redraw after this one so we keep a consistent framerate
                state.get_window().request_redraw();

                if let Some(audio) = &mut state.audio {
                    // Request another batch of fft work after this one
                    audio::worker::submit_work(&audio.tx);
                    audio.last_bins = audio
                        .bins
                        .lock()
                        .unwrap()
                        .iter()
                        .map(Clone::clone)
                        .collect::<Vec<_>>()
                        .try_into()
                        .expect("wrong number of bins");
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
                        repeat,
                        ..
                    },
                ..
            } => {
                if state.handle_music_key(key, repeat) {
                    return;
                }
                state.pipeline.handle_keypress(&state.queue, key);
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.close_requested {
            event_loop.exit();
        }
    }
}

mod flags {
    use std::path::PathBuf;

    xflags::xflags! {
        cmd main {
            optional --music file: PathBuf
            optional --settings file: PathBuf
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
        close_requested: false,
    };
    event_loop.run_app(&mut app).unwrap();
}
