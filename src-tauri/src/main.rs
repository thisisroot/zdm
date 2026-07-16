// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    linux_appimage_render_fix();

    app_lib::run();
}

/// Work around a WebKitGTK rendering failure that only affects the **AppImage**
/// build on modern Linux desktops (Wayland + recent Mesa — e.g. Intel Arc,
/// NVIDIA). Symptoms: the window opens blank, or the process aborts with
/// `Could not create default EGL display: EGL_BAD_PARAMETER`.
///
/// Cause: the AppImage bundles its own `libwayland-client`, built on an old CI
/// base. When the host's up-to-date Mesa EGL is loaded against that stale
/// bundled copy, `eglGetPlatformDisplay(EGL_PLATFORM_WAYLAND)` fails. The cure
/// is to force the **host** `libwayland-client` ahead of the bundled one via
/// `LD_PRELOAD`. `LD_PRELOAD` is only honored at process start, so we set it and
/// re-exec ourselves exactly once (guarded by a sentinel env var).
///
/// We additionally disable WebKit's DMA-BUF renderer, which independently blanks
/// on some GPU/driver combos and is the standard Tauri-on-Linux mitigation.
///
/// This only runs inside an AppImage (the type-2 runtime exports `APPDIR`), so
/// `.deb` / `.rpm` / `cargo run` builds — which already link the host libraries
/// — are left completely untouched.
#[cfg(target_os = "linux")]
fn linux_appimage_render_fix() {
    use std::os::unix::process::CommandExt;
    use std::path::Path;

    // Only inside an AppImage. deb/rpm/dev builds use host libs already.
    if std::env::var_os("APPDIR").is_none() && std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    // The re-exec below inherits our environment; bail if we've already run.
    if std::env::var_os("ZDM_RENDER_FIX_APPLIED").is_some() {
        return;
    }
    std::env::set_var("ZDM_RENDER_FIX_APPLIED", "1");

    // Blank-screen guard for the DMA-BUF renderer (read by WebKitGTK when the
    // webview is created). Still GPU-accelerated via the fallback path.
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    // The EGL_BAD_PARAMETER / blank-window bug is Wayland-specific: on X11 the
    // bundled libwayland-client is never used for display, so there is nothing
    // to preload and no need to re-exec.
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }

    // Find the host libwayland-client — never the one bundled under $APPDIR.
    let appdir = std::env::var("APPDIR").unwrap_or_default();
    let host_lib = [
        "/usr/lib/x86_64-linux-gnu/libwayland-client.so.0",
        "/usr/lib64/libwayland-client.so.0",
        "/usr/lib/libwayland-client.so.0",
        "/lib/x86_64-linux-gnu/libwayland-client.so.0",
    ]
    .into_iter()
    .find(|p| (appdir.is_empty() || !p.starts_with(&appdir)) && Path::new(p).exists());

    // No host copy found (unusual) — run as-is rather than re-exec pointlessly.
    let Some(lib) = host_lib else {
        return;
    };

    let preload = match std::env::var("LD_PRELOAD") {
        Ok(existing) if !existing.is_empty() => format!("{lib}:{existing}"),
        _ => lib.to_string(),
    };
    std::env::set_var("LD_PRELOAD", preload);

    // LD_PRELOAD only takes effect on a freshly started process, so re-exec the
    // same binary with the same arguments and our augmented environment.
    if let Ok(exe) = std::env::current_exe() {
        let err = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .exec();
        // exec() only returns on failure; continue in-process as a last resort.
        eprintln!("zdm: AppImage render-fix re-exec failed: {err}");
    }
}
