//! Starting and stopping the browser engine inside this process.

use std::path::{Path, PathBuf};

use cef::args::Args;
use cef::*;

use crate::app::EngineApp;

/// The running browser engine; it stays on the thread that started it and is shut down when dropped.
pub(crate) struct Engine;

impl Engine {
    /// Starts the engine, keeping profiles, cookies and caches under `profile`.
    pub(crate) fn start(profile: &Path) -> Result<Self, String> {
        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
        let args = Args::new();
        let switches = vec![
            "noerrdialogs".to_owned(),
            "use-mock-keychain".to_owned(),
            "ozone-platform=x11".to_owned(),
            "use-angle=gl-egl".to_owned(),
            "disable-background-networking".to_owned(),
            "disable-component-update".to_owned(),
            "disable-features=MediaRouter".to_owned(),
        ];
        let mut app = EngineApp::new(switches);
        let runtime = runtime_dir();
        let settings = Settings {
            resources_dir_path: runtime
                .as_deref()
                .and_then(Path::to_str)
                .map_or_else(CefString::default, CefString::from),
            locales_dir_path: runtime
                .as_deref()
                .and_then(|dir| dir.join("locales").to_str().map(CefString::from))
                .unwrap_or_default(),
            windowless_rendering_enabled: true as _,
            external_message_pump: true as _,
            root_cache_path: CefString::from(profile.to_str().ok_or("bad profile path")?),
            log_severity: LogSeverity::WARNING,
            ..Default::default()
        };
        let started = initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if started != 1 {
            return Err("the engine failed to initialize".to_owned());
        }
        Ok(Self)
    }

    /// Lets the engine do the work it has queued.
    pub(crate) fn pump(&self) {
        do_message_loop_work();
    }

    /// Gives the engine `time` to finish what is queued, such as closing pages, before it is shut down.
    pub(crate) fn settle(&self, time: std::time::Duration) {
        let until = std::time::Instant::now() + time;
        while std::time::Instant::now() < until {
            do_message_loop_work();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

impl Drop for Engine {
    /// Shuts the engine down.
    fn drop(&mut self) {
        shutdown();
    }
}

/// Where the engine's runtime files are: beside this program, in `CEF_PATH`, or in the per-user cache, whichever has them first.
fn runtime_dir() -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let configured = std::env::var_os("CEF_PATH").map(PathBuf::from);
    let cached = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/cef"));
    beside
        .into_iter()
        .chain(configured)
        .chain(cached)
        .find(|dir| dir.join("icudtl.dat").exists())
}

/// Runs this process as one of the engine's own helpers when it is one, and gives the code to exit with; gives nothing in the browser process.
///
/// The engine starts its helpers by running this program again with a
/// `--type` argument.
pub(crate) fn run_helper() -> Option<i32> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let command = args.as_cmd_line()?;
    if command.has_switch(Some(&CefString::from("type"))) != 1 {
        return None;
    }
    let mut app = EngineApp::new(Vec::new());
    Some(execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    ))
}
