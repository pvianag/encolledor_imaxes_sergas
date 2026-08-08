#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, IconData};
use sergas_zip_shrinker::app::ShrinkApp;
use std::path::PathBuf;
use std::sync::Arc;

fn load_app_icon() -> Option<Arc<IconData>> {
    let img = image::load_from_memory(include_bytes!("../assets/app_icon.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(Arc::new(IconData {
        rgba: img.into_raw(),
        width,
        height,
    }))
}

fn crash_log_path() -> PathBuf {
    std::env::temp_dir().join("sergas-zip-shrinker-crash.log")
}

fn write_crash_log(message: &str) {
    let path = crash_log_path();
    let body = format!(
        "Sergas ZIP Shrinker {}\n{}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        chrono_like_now(),
        message
    );
    let _ = std::fs::write(&path, body);
}

fn chrono_like_now() -> String {
    // Keep deps minimal: local time via system clock formatting is enough for a crash log.
    format!("{:?}", std::time::SystemTime::now())
}

#[cfg(windows)]
fn show_windows_error(message: &str) {
    use std::os::windows::ffi::OsStrExt;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let log = crash_log_path();
    let full = format!(
        "{message}\n\nDetails were also written to:\n{}",
        log.display()
    );
    let text = wide(&full);
    let caption = wide("Sergas ZIP Shrinker");

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            lp_text: *const u16,
            lp_caption: *const u16,
            u_type: u32,
        ) -> i32;
    }

    // MB_OK | MB_ICONERROR
    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_windows_error(_message: &str) {}

fn report_fatal(message: &str) {
    eprintln!("{message}");
    write_crash_log(message);
    show_windows_error(message);
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".into());
        report_fatal(&format!("The application crashed.\n\n{payload}\n\n at {loc}"));
        default_hook(info);
    }));
}

fn run_with(renderer: eframe::Renderer, viewport: egui::ViewportBuilder) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport,
        renderer,
        ..Default::default()
    };

    eframe::run_native(
        "Sergas ZIP Shrinker",
        options,
        Box::new(|cc| Ok(Box::new(ShrinkApp::new(cc)))),
    )
}

fn run() -> Result<(), String> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([860.0, 780.0])
        .with_min_inner_size([720.0, 640.0])
        .with_title("Sergas ZIP Shrinker")
        .with_drag_and_drop(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    // Windows: prefer wgpu → DirectX 12 (OpenGL via glow often missing under RDP/VMs).
    // Fall back to glow if wgpu cannot find an adapter.
    #[cfg(windows)]
    {
        match run_with(eframe::Renderer::Wgpu, viewport.clone()) {
            Ok(()) => return Ok(()),
            Err(wgpu_err) => {
                eprintln!("wgpu startup failed ({wgpu_err}); retrying with OpenGL (glow)…");
                match run_with(eframe::Renderer::Glow, viewport) {
                    Ok(()) => return Ok(()),
                    Err(glow_err) => {
                        return Err(format!("wgpu: {wgpu_err}\nglow: {glow_err}"));
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        run_with(eframe::Renderer::Glow, viewport).map_err(|e| e.to_string())
    }
}

fn main() {
    install_panic_hook();
    if let Err(err) = run() {
        report_fatal(&format!(
            "The application failed to start.\n\n{err}\n\nIf this persists, check that your GPU drivers are working, and that Microsoft Defender did not quarantine the file."
        ));
        std::process::exit(1);
    }
}
