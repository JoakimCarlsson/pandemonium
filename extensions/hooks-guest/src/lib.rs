//! The module behind the sample language extension: it answers the language calls the host makes for a
//! language the extension carries. It is written against the typed contract of `pm-plugin-api`, so what
//! crosses the boundary is checked by the compiler rather than written by hand.
//!
//! It stands in for a language with decimal versions: the latest version of every dependency is `2.31`, a
//! requirement is behind it when it is lower as a decimal number (`2.2207` is behind `2.31`: it is the
//! fraction that decides, as it does for CPAN), and the server and the tasks are a program and a task it names.

use pm_plugin_api::{LanguageCall, LanguageReply, Launch, TaskItem, ToolchainItem, pack};

/// Hands the host `length` bytes of this module's memory to write into.
#[unsafe(no_mangle)]
pub extern "C" fn pm_alloc(length: usize) -> *mut u8 {
    let mut room = Vec::<u8>::with_capacity(length.max(1));
    let pointer = room.as_mut_ptr();
    std::mem::forget(room);
    pointer
}

/// The decimal number `text` is, as its whole part and its fraction digits.
fn decimal(text: &str) -> Option<(u64, String)> {
    let (whole, fraction) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
    Some((whole.parse().ok()?, fraction.to_owned()))
}

/// Whether the decimal version `requirement` is lower than `latest`.
fn is_behind(requirement: &str, latest: &str) -> bool {
    let (Some((rw, rf)), Some((lw, lf))) = (decimal(requirement), decimal(latest)) else {
        return false;
    };
    let width = rf.len().max(lf.len());
    (rw, format!("{rf:0<width$}")) < (lw, format!("{lf:0<width$}"))
}

/// What the host asked, answered.
fn answer(call: LanguageCall) -> LanguageReply {
    match call {
        LanguageCall::ServerCommand { .. } => LanguageReply::Command(Launch {
            program: "perl-language-server".to_owned(),
            args: vec!["--stdio".to_owned()],
            ..Launch::default()
        }),
        LanguageCall::LatestVersion { .. } => LanguageReply::Version(Some("2.31".to_owned())),
        LanguageCall::Behind {
            requirement,
            latest,
            ..
        } => LanguageReply::Replacement(is_behind(&requirement, &latest).then_some(latest)),
        LanguageCall::Tasks { .. } => LanguageReply::Tasks(vec![TaskItem {
            name: "prove".to_owned(),
            command: "prove -lr t".to_owned(),
            group: "test".to_owned(),
            ..TaskItem::default()
        }]),
        LanguageCall::Toolchains { .. } => LanguageReply::Toolchains(vec![ToolchainItem {
            id: "perlbrew 5.40".to_owned(),
            name: "perlbrew 5.40".to_owned(),
            ..ToolchainItem::default()
        }]),
    }
}

/// Answers a language call: reads it from the memory the host wrote it to and leaves the reply there.
#[unsafe(no_mangle)]
pub extern "C" fn pm_language(pointer: *const u8, length: usize) -> i64 {
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let reply = match serde_json::from_slice::<LanguageCall>(bytes) {
        Ok(call) => answer(call),
        Err(_) => LanguageReply::Unsupported,
    };
    let bytes = serde_json::to_vec(&reply)
        .unwrap_or_default()
        .into_boxed_slice();
    let length = bytes.len() as u32;
    let pointer = Box::leak(bytes).as_ptr() as u32;
    pack(pointer, length)
}
