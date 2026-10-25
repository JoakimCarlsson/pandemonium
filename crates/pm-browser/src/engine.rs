//! Starting and stopping the browser engine.

use std::path::{Path, PathBuf};

use cef::args::Args;
use cef::*;

use crate::app::EngineApp;
use crate::error::Error;

/// How the engine renders.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Extra Chromium command-line switches, as `name` or `name=value`.
    pub switches: Vec<String>,
    /// Hand frames over as GPU shared textures instead of CPU pixel buffers.
    pub shared_texture: bool,
    /// The local port the pages' developer tools are served on, or none to serve no tools.
    pub debugging_port: Option<u16>,
}

/// The running browser engine; pages are opened inside it as [`crate::View`]s.
///
/// It must outlive every view and stays on the thread that started it.
pub struct Engine {
    /// Whether pages are painted into shared textures.
    pub(crate) shared_texture: bool,
    /// The local port the pages' developer tools are served on.
    debugging_port: Option<u16>,
}

impl Engine {
    /// Starts the engine in this process, keeping profiles, cookies and caches under `cache_dir`.
    ///
    /// Call [`run_helper`] first thing in `main` so the processes the engine
    /// starts by running this executable again do their work and exit.
    pub fn start(cache_dir: &Path, options: &Options) -> Result<Self, Error> {
        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
        let args = Args::new();
        let mut switches = vec!["noerrdialogs".to_owned(), "use-mock-keychain".to_owned()];
        switches.extend(options.switches.iter().cloned());
        let mut app = EngineApp::new(switches);
        if std::env::var_os("FONTCONFIG_PATH").is_none() && Path::new("/etc/fonts").is_dir() {
            // SAFETY: the engine starts before any other thread reads the environment.
            unsafe { std::env::set_var("FONTCONFIG_PATH", "/etc/fonts") };
        }
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
            root_cache_path: CefString::from(cache_dir.to_str().ok_or(Error::CachePath)?),
            log_severity: LogSeverity::WARNING,
            remote_debugging_port: options.debugging_port.map_or(0, i32::from),
            ..Default::default()
        };
        let started = initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if started != 1 {
            return Err(Error::Engine("initialization failed"));
        }
        Ok(Self {
            shared_texture: options.shared_texture,
            debugging_port: options.debugging_port,
        })
    }

    /// The local port the pages' developer tools are served on, when they are.
    pub fn debugging_port(&self) -> Option<u16> {
        self.debugging_port
    }

    /// Gives the engine `time` to finish what is queued, such as closing pages, before it is shut down.
    pub fn settle(&self, time: std::time::Duration) {
        let until = std::time::Instant::now() + time;
        while std::time::Instant::now() < until {
            do_message_loop_work();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Lets the engine do the work it has queued; call it often from the application's loop.
    pub fn pump(&self) {
        do_message_loop_work();
    }
}

impl Drop for Engine {
    /// Shuts the engine down.
    fn drop(&mut self) {
        shutdown();
    }
}

/// Where the engine's runtime files are: beside the executable, in `CEF_PATH`, or in the per-user cache, whichever has them first.
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

/// Runs this process as one of the engine's helpers when it is one, and gives the code to exit with; gives nothing in the application's own process.
///
/// The engine starts its helpers by running this executable again with a
/// `--type` argument, so call this first thing in `main`.
pub fn run_helper() -> Option<i32> {
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
