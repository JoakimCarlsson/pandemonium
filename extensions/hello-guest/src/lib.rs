//! The module behind the sample extension's `branch` command: it asks the host to run
//! `git rev-parse --abbrev-ref HEAD` in the project and answers with a prompt that names
//! the branch. It is refused, and says so, when the reader has not allowed it to run
//! programs.

#[link(wasm_import_module = "pandemonium")]
unsafe extern "C" {
    /// Sends the JSON request at `pointer` of `length` bytes to the host, which answers with a
    /// pointer and a length packed into one `i64`, the pointer in the high half.
    #[link_name = "host"]
    fn host(pointer: *const u8, length: usize) -> i64;
}

/// Hands the host `length` bytes of this module's memory to write into.
#[unsafe(no_mangle)]
pub extern "C" fn pm_alloc(length: usize) -> *mut u8 {
    let mut room = Vec::<u8>::with_capacity(length.max(1));
    let pointer = room.as_mut_ptr();
    std::mem::forget(room);
    pointer
}

/// The bytes a pointer and a length packed into `packed` name.
fn unpacked(packed: i64) -> String {
    let pointer = (packed >> 32) as u32 as usize as *const u8;
    let length = packed as u32 as usize;
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// `text` left in memory for the host to read, as a packed pointer and length.
fn packed(text: String) -> i64 {
    let bytes = text.into_bytes().into_boxed_slice();
    let length = bytes.len();
    let pointer = Box::leak(bytes).as_ptr() as usize;
    ((pointer as i64) << 32) | length as i64
}

/// `text` as a JSON string literal.
fn quoted(text: &str) -> String {
    let mut written = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => written.push_str("\\\""),
            '\\' => written.push_str("\\\\"),
            '\n' => written.push_str("\\n"),
            other => written.push(other),
        }
    }
    written.push('"');
    written
}

/// Runs a command: asks the host for the branch the project is on and answers with a prompt
/// that reviews it, or with the reason the host refused.
#[unsafe(no_mangle)]
pub extern "C" fn pm_command(_pointer: *const u8, _length: usize) -> i64 {
    let request = r#"{"op":"process","program":"git","args":["rev-parse","--abbrev-ref","HEAD"]}"#;
    let reply = unpacked(unsafe { host(request.as_ptr(), request.len()) });
    if let Some(at) = reply.find("\"stdout\":\"") {
        let rest = &reply[at + 10..];
        let branch = rest.split("\\n").next().unwrap_or_default().trim();
        return packed(format!(
            "{{\"prompt\":{}}}",
            quoted(&format!("Review the changes on the branch {branch} against main."))
        ));
    }
    let reason = reply
        .split("\"error\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("the host did not answer");
    packed(format!("{{\"message\":{}}}", quoted(reason)))
}
