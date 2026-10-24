#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cache;
mod codec;
mod folder;
mod hud;
mod ipc;
mod pipeline;
mod renderer;
mod tiling;
mod config;

use app::App;
use codec::LoadedFile;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopBuilder;
use winit::window::WindowBuilder;

#[cfg(target_os = "windows")]
extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
    fn SetForegroundWindow(hWnd: isize) -> i32;
}
const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<dyn Any>",
            },
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let report = format!(
            "Photon Viewer encountered an unexpected error:\n\nPanic: {}\nLocation: {}\n\nA diagnostic log has been written to photon_crash.log.",
            msg, location
        );
        eprintln!("{}", report);

        if let Ok(mut f) = std::fs::File::create("photon_crash.log") {
            use std::io::Write;
            let _ = writeln!(f, "Crash timestamp: {:?}", std::time::SystemTime::now());
            let _ = writeln!(f, "{}", report);
        }
        if let Ok(temp) = std::env::var("TEMP") {
            let temp_log = std::path::PathBuf::from(temp).join("photon_crash.log");
            if let Ok(mut f) = std::fs::File::create(temp_log) {
                use std::io::Write;
                let _ = writeln!(f, "Crash timestamp: {:?}", std::time::SystemTime::now());
                let _ = writeln!(f, "{}", report);
            }
        }

        #[cfg(target_os = "windows")]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            let wide_title: Vec<u16> = OsStr::new("Photon Viewer - Fatal Error\0").encode_wide().collect();
            let wide_msg: Vec<u16> = OsStr::new(&report).encode_wide().chain(Some(0)).collect();
            extern "system" {
                fn MessageBoxW(hWnd: isize, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
            }
            unsafe {
                MessageBoxW(0, wide_msg.as_ptr(), wide_title.as_ptr(), 0x00000010 /* MB_ICONERROR */);
            }
        }
    }));

    let args: Vec<String> = env::args().collect();

    // Headless verification and benchmark mode:
    // photon_viewer --verify <path>
    if args.len() >= 3 && args[1] == "--verify" {
        #[cfg(target_os = "windows")]
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
        let path = PathBuf::from(&args[2]);
        let start = Instant::now();
        match LoadedFile::open(&path) {
            Ok(file) => {
                let fmt = file.format;
                let open_time = start.elapsed().as_micros();
                match file.decode() {
                    Ok(decoded) => {
                        let total_time = start.elapsed().as_micros();
                        println!("VERIFY_SUCCESS");
                        println!("Format: {}", fmt.name());
                        println!("Resolution: {}x{}", decoded.width, decoded.height);
                        println!("Orientation: {}", file.orientation);
                        println!("MemoryMappedOpen: {:.3}ms", open_time as f64 / 1000.0);
                        println!("DecodeTime: {:.3}ms", decoded.decode_duration_us as f64 / 1000.0);
                        println!("TotalTimeToPixels: {:.3}ms", total_time as f64 / 1000.0);
                        return;
                    }
                    Err(e) => {
                        eprintln!("VERIFY_DECODE_ERROR: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("VERIFY_OPEN_ERROR: {}", e);
                std::process::exit(1);
            }
        }
    }

    let multi_instance = args.iter().any(|a| a == "--multi-instance");
    let initial_file = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from);

    // Single-instance enforcement (default):
    // If an instance is already running, forward the image path to it and exit immediately.
    if !multi_instance {
        if ipc::try_send_to_existing_instance(initial_file.as_deref()) {
            return;
        }
    }

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let event_loop = EventLoopBuilder::<PathBuf>::with_user_event()
        .build()
        .expect("Failed to create event loop");
    let proxy = event_loop.create_proxy();

    if !multi_instance {
        ipc::start_ipc_server(proxy);
    }

    let mut builder = WindowBuilder::new()
        .with_title("Photon Viewer")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
        .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 240.0));

    if let Some(icon) = load_app_icon() {
        builder = builder.with_window_icon(Some(icon));
    }

    let window = builder
        .build(&event_loop)
        .expect("Failed to create window");

    let window = Arc::new(window);

    let mut app = match pollster::block_on(App::new(window.clone(), initial_file)) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Initialization error: {}", e);
            #[cfg(target_os = "windows")]
            {
                use std::ffi::OsStr;
                use std::os::windows::ffi::OsStrExt;
                let wide_title: Vec<u16> = OsStr::new("Photon Viewer - Graphics Adapter Error\0").encode_wide().collect();
                let wide_msg: Vec<u16> = OsStr::new(&format!("{}\n\nPlease verify your graphics driver installation.", e)).encode_wide().chain(Some(0)).collect();
                extern "system" {
                    fn MessageBoxW(hWnd: isize, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
                }
                unsafe {
                    MessageBoxW(0, wide_msg.as_ptr(), wide_title.as_ptr(), 0x00000010 /* MB_ICONERROR */);
                }
            }
            std::process::exit(1);
        }
    };

    event_loop
        .run(move |event, target| match event {
            Event::UserEvent(path) => {
                window.set_minimized(false);
                window.focus_window();
                #[cfg(target_os = "windows")]
                {
                    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    if let Ok(handle) = window.window_handle() {
                        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                            unsafe {
                                SetForegroundWindow(win32_handle.hwnd.get());
                            }
                        }
                    }
                }
                app.open_file_and_scan_folder(path);
                window.request_redraw();
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !app.handle_event(event) {
                    match event {
                        WindowEvent::CloseRequested => target.exit(),
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                app.update_animation();
                if let Some(deadline) = app.animation_deadline() {
                    target.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(deadline));
                } else {
                    target.set_control_flow(winit::event_loop::ControlFlow::Wait);
                }
            }
            _ => {}
        })
        .expect("Event loop error");
}

fn load_app_icon() -> Option<winit::window::Icon> {
    let icon_bytes = include_bytes!("../assets/logo.png");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img
            .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (w, h) = rgba.dimensions();
        winit::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
    } else {
        None
    }
}

