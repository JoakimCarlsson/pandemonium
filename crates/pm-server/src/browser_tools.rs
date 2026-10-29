//! The browser's tools, offered to agents as an MCP server on the loopback address and run by a window.
//!
//! An agent calls a tool over HTTP; the server sends the call to the client that
//! last said it can run them, waits for the answer, and gives it back.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use pm_protocol::ServerMessage;
use serde_json::{Value, json};

use crate::core::Connection;

/// How long a call waits for a window to answer.
const PATIENCE: Duration = Duration::from_secs(90);

/// The most bytes of a request that are read.
const MOST: usize = 1 << 20;

/// What a window answered a call with.
pub(crate) struct Reply {
    /// Whether the tool worked.
    pub(crate) ok: bool,
    /// What it says.
    pub(crate) text: String,
    /// A picture it took, as PNG.
    pub(crate) image: Vec<u8>,
}

/// What the server knows of the tools.
struct Tools {
    /// The address agents are given and the token that lets them in, once the endpoint listens.
    endpoint: OnceLock<(String, String)>,
    /// The connections that can run the tools, oldest first.
    runners: Mutex<Vec<Arc<Connection>>>,
    /// The calls waiting for an answer.
    pending: Mutex<HashMap<u64, Sender<Reply>>>,
    /// The number the next call gets.
    next: AtomicU64,
}

/// The one set of tools of this server.
static TOOLS: LazyLock<Tools> = LazyLock::new(|| Tools {
    endpoint: OnceLock::new(),
    runners: Mutex::new(Vec::new()),
    pending: Mutex::new(HashMap::new()),
    next: AtomicU64::new(1),
});

/// The address and token of the tools, to be given to an agent when it starts, once any window has said it can run them.
/// An agent only learns of its tools when it starts, so they are kept on offer while no window is connected: an agent started as a window restarts or reconnects would otherwise never get them, and a call made then says that no window can run them.
pub(crate) fn offered() -> Option<(String, String)> {
    TOOLS.endpoint.get().cloned()
}

/// Notes that `connection` can, or can no longer, run the tools, and starts the endpoint when the first one can.
pub(crate) fn set_available(connection: &Arc<Connection>, available: bool) {
    if available {
        ensure_started();
    }
    if let Ok(mut runners) = TOOLS.runners.lock() {
        runners.retain(|held| !Arc::ptr_eq(held, connection));
        if available {
            runners.push(connection.clone());
        }
    }
}

/// Forgets `connection`, which went away.
pub(crate) fn forget(connection: &Arc<Connection>) {
    set_available(connection, false);
}

/// Hands the answer of a window to the call waiting for it.
pub(crate) fn answer(call: u64, reply: Reply) {
    let sender = TOOLS.pending.lock().ok().and_then(|mut p| p.remove(&call));
    if let Some(sender) = sender {
        let _ = sender.send(reply);
    }
}

/// Starts listening, once.
fn ensure_started() {
    if TOOLS.endpoint.get().is_some() {
        return;
    }
    let mut secret = [0u8; 24];
    if getrandom::fill(&mut secret).is_err() {
        return;
    }
    let token: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
        return;
    };
    let Ok(local) = listener.local_addr() else {
        return;
    };
    if TOOLS
        .endpoint
        .set((
            format!("http://127.0.0.1:{}/mcp", local.port()),
            token.clone(),
        ))
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let token = token.clone();
            std::thread::spawn(move || serve(stream, &token));
        }
    });
}

/// Whether the two texts are the same, taking as long to say so whatever they hold.
fn same(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// Writes an HTTP answer with `status` and `body`, and ends the connection.
fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// Serves one request of an agent.
fn serve(mut stream: TcpStream, token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || !line.starts_with("POST /mcp") {
        respond(&mut stream, "404 Not Found", "{}");
        return;
    }
    let (mut length, mut allowed) = (0usize, false);
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse().unwrap_or(0),
            "authorization" => {
                allowed = value
                    .strip_prefix("Bearer ")
                    .is_some_and(|given| same(given, token));
            }
            _ => {}
        }
    }
    if !allowed {
        respond(&mut stream, "401 Unauthorized", "{}");
        return;
    }
    if length > MOST {
        respond(&mut stream, "413 Payload Too Large", "{}");
        return;
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        respond(&mut stream, "400 Bad Request", "{}");
        return;
    };
    match handle(&message) {
        Some(answer) => respond(&mut stream, "200 OK", &answer.to_string()),
        None => respond(&mut stream, "202 Accepted", "{}"),
    }
}

/// The answer to one JSON-RPC message, or nothing when it is a notification.
fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => json!({
            "protocolVersion": message["params"]["protocolVersion"]
                .as_str()
                .unwrap_or("2025-03-26"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pandemonium-browser", "version": env!("CARGO_PKG_VERSION") },
        }),
        "tools/list" => json!({ "tools": definitions() }),
        "tools/call" => call_tool(&message["params"]),
        "ping" => json!({}),
        other => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("no method {other}") },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// Runs the tool a `tools/call` names in a window and describes what came of it.
fn call_tool(params: &Value) -> Value {
    let Some(tool) = params["name"].as_str() else {
        return failure("no tool was named");
    };
    if !definitions()
        .iter()
        .any(|definition| definition["name"] == tool)
    {
        return failure(&format!("there is no tool {tool}"));
    }
    let arguments = params["arguments"].to_string();
    match run(tool, &arguments) {
        Ok(reply) => {
            let mut content = vec![json!({ "type": "text", "text": reply.text })];
            if !reply.image.is_empty() {
                content.push(json!({
                    "type": "image",
                    "data": base64(&reply.image),
                    "mimeType": "image/png",
                }));
            }
            json!({ "content": content, "isError": !reply.ok })
        }
        Err(reason) => failure(&reason),
    }
}

/// A tool result that says the call failed, for `reason`.
fn failure(reason: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": reason }], "isError": true })
}

/// Sends the call to the window that can run it and waits for the answer.
fn run(tool: &str, arguments: &str) -> Result<Reply, String> {
    let runner = TOOLS
        .runners
        .lock()
        .ok()
        .and_then(|runners| runners.last().cloned())
        .ok_or("no window with the embedded browser turned on is connected")?;
    let call = TOOLS.next.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = channel();
    if let Ok(mut pending) = TOOLS.pending.lock() {
        pending.insert(call, sender);
    }
    let sent = runner.outbox.send(ServerMessage::BrowserCall {
        call,
        tool: tool.to_owned(),
        arguments: arguments.to_owned(),
    });
    if sent.is_err() {
        if let Ok(mut pending) = TOOLS.pending.lock() {
            pending.remove(&call);
        }
        return Err("the window with the browser went away".to_owned());
    }
    receiver.recv_timeout(PATIENCE).map_err(|_| {
        if let Ok(mut pending) = TOOLS.pending.lock() {
            pending.remove(&call);
        }
        "the browser did not answer in time".to_owned()
    })
}

/// `bytes` as standard base64.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = chunk.iter().enumerate().fold(0u32, |held, (place, byte)| {
            held | (u32::from(*byte) << (16 - 8 * place))
        });
        for place in 0..4 {
            if place <= chunk.len() {
                text.push(char::from(
                    TABLE[((triple >> (18 - 6 * place)) & 63) as usize],
                ));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// A tool the agents are offered: its name, what it does and what it takes.
fn tool(name: &str, description: &str, properties: &Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

/// The tools the agents are offered.
fn definitions() -> Vec<Value> {
    let target = json!({
        "ref": { "type": "integer", "description": "A number from the last browser_snapshot." },
        "selector": { "type": "string", "description": "A CSS selector." },
    });
    let mut tools = vec![
        tool(
            "browser_navigate",
            "Open an address in the browser pane and wait for it to load and settle, then say where it ended up, its title and its HTTP status.",
            &json!({ "url": { "type": "string" } }),
            &["url"],
        ),
        tool(
            "browser_snapshot",
            "Describe the page: its address, title and a numbered list of the elements that can be clicked or typed in, with their text.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_screenshot",
            "Take a picture of the page once the requests it has under way are done: the viewport by default, the whole scrollable page with `full_page` (cut at 16000 pixels), or one element given by `ref` or `selector`. `full_page` cannot be combined with an element.",
            &json!({
                "full_page": { "type": "boolean", "description": "Capture the whole page, not only the viewport." },
                "ref": target["ref"],
                "selector": target["selector"],
            }),
            &[],
        ),
        tool(
            "browser_click",
            "Click an element, given by `ref` or `selector`, or a point given by `x` and `y` in the page.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "x": { "type": "number" },
                "y": { "type": "number" },
            }),
            &[],
        ),
        tool(
            "browser_type",
            "Type text into a text field, given by `ref` or `selector`; `submit` presses Enter afterwards. Lists and boxes cannot be typed in.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "text": { "type": "string" },
                "submit": { "type": "boolean" },
            }),
            &["text"],
        ),
        tool(
            "browser_select_option",
            "Choose an option of a list (a `select`), given by `ref` or `selector`, by its `value` or its text.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "value": { "type": "string" },
            }),
            &["value"],
        ),
        tool(
            "browser_press_key",
            "Press a key, such as `Escape`, `Tab`, `Enter`, `ArrowDown`, `F5` or `a`, with `modifiers` held (some of `shift`, `control`, `alt`, `meta`); an element given by `ref` or `selector` is focused first.",
            &json!({
                "key": { "type": "string" },
                "modifiers": { "type": "array", "items": { "type": "string" } },
                "ref": target["ref"],
                "selector": target["selector"],
            }),
            &["key"],
        ),
        tool(
            "browser_scroll",
            "Scroll the page, or the element given by `ref` or `selector`: bring it into view when no `direction` is given, or move `direction` (`up`, `down`, `left`, `right`, `top` or `bottom`) by `amount` pixels, a screen when not given. Says where it ended up.",
            &json!({
                "direction": { "type": "string" },
                "amount": { "type": "number" },
                "ref": target["ref"],
                "selector": target["selector"],
            }),
            &[],
        ),
        tool(
            "browser_back",
            "Go back one page in the history and wait for it to load and settle.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_forward",
            "Go forward one page in the history and wait for it to load and settle.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_reload",
            "Load the page again and wait for it to load and settle.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_evaluate",
            "Run a JavaScript expression in the page and give back its value as JSON.",
            &json!({ "expression": { "type": "string" } }),
            &["expression"],
        ),
        tool(
            "browser_console",
            "The messages and errors the page wrote to its console since it was opened (the latest 200 are kept), one per line as `[level] text`. `level` keeps only some levels (`error`, `warning`, `info`, `log`, `debug`; several separated by commas). `clear` true forgets every message after this answer.",
            &json!({
                "level": { "type": "string", "description": "Levels to show: some of error, warning, info, log, debug, separated by commas." },
                "clear": { "type": "boolean", "description": "Forget all recorded messages after returning the matching ones." },
            }),
            &[],
        ),
        tool(
            "browser_network",
            "The requests the page made since it was opened (the latest 200 are kept), one per line as `#number METHOD url -> status`. `filter` keeps requests whose address contains the text, whose method is it, whose status is it, or whose status class it names (`4xx`; `failed` for failures). `clear` true forgets every recorded request after this answer. `request` gives one request in full by its number: type, size on the wire, timing, request and response headers (Authorization, Cookie and Set-Cookie values are redacted) and, for text types such as HTML, JSON and scripts, the response body cut to 8000 characters. Limits: headers are those the engine reports before it adds its own (cookies it attaches are not shown); the body is only there while the engine still holds it, so it can be missing after navigation or for big responses.",
            &json!({
                "filter": { "type": "string", "description": "Part of the address, a method, a status such as 404, a class such as 4xx, or failed." },
                "clear": { "type": "boolean", "description": "Forget all recorded requests after returning the matching ones." },
                "request": { "type": "integer", "description": "The number of one request from the list, to see its headers, size, timing and body." },
            }),
            &[],
        ),
        tool(
            "browser_wait",
            "Wait until `text` is on the page or `selector` matches, or `timeout_ms` passes.",
            &json!({
                "text": { "type": "string" },
                "selector": { "type": "string" },
                "timeout_ms": { "type": "integer" },
            }),
            &[],
        ),
        tool(
            "browser_handle_dialog",
            "Answer the JavaScript dialog (`alert` or `confirm`) that the page has open. While one is open the page is stopped and the other browser tools refuse to run; it is dismissed by itself after 60 seconds. Other tools' answers say when a dialog is open or closed. `prompt` dialogs are cancelled by the browser at once and cannot be answered, so `prompt_text` has no effect today. `beforeunload` prompts are not answered here: a tool that navigates leaves the page anyway.",
            &json!({
                "accept": { "type": "boolean", "description": "true presses OK, false presses Cancel." },
                "prompt_text": { "type": "string", "description": "Text for a prompt dialog; see the description." },
            }),
            &["accept"],
        ),
        tool(
            "browser_hover",
            "Move the pointer over an element, given by `ref` or `selector`, or a point given by `x` and `y`, so the page shows what hovering shows, such as menus and tooltips.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "x": { "type": "number" },
                "y": { "type": "number" },
            }),
            &[],
        ),
        tool(
            "browser_drag",
            "Press the pointer at `from`, move it to `to` and let go. Each end is an element (`ref` or `selector`) or a point (`x` and `y`). Works for HTML drag and drop and for drags the page follows with mouse events, such as sliders.",
            &json!({
                "from": drag_end(&target, "Where the drag starts."),
                "to": drag_end(&target, "Where the drag ends."),
            }),
            &["from", "to"],
        ),
        tool(
            "browser_fill_form",
            "Fill in several fields in one call, in order, stopping at the first that fails. A text field takes a string as `value` (as browser_type does, replacing what is there), a list (`select`) takes the option's value or text (as browser_select_option does), a checkbox or radio button takes `true` to tick it or `false` to clear it.",
            &json!({
                "fields": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 50,
                    "items": {
                        "type": "object",
                        "properties": {
                            "ref": target["ref"],
                            "selector": target["selector"],
                            "value": { "type": ["string", "boolean"] },
                        },
                        "required": ["value"],
                    },
                },
            }),
            &["fields"],
        ),
        tool(
            "browser_highlight",
            "Outline an element, given by `ref` or `selector`, in the browser pane for a moment so the person watching sees what you mean. It scrolls the element into view and returns at once; the outline fades after `duration_ms` (default 2000, from 200 to 10000), and a screenshot taken meanwhile shows it.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "duration_ms": { "type": "integer" },
                "label": { "type": "string", "description": "A short caption shown on the outline." },
            }),
            &[],
        ),
        tool(
            "browser_get_bounding_box",
            "Say where an element, given by `ref` or `selector`, is, without moving the page: `x`, `y`, `width` and `height` in CSS pixels from the top left of the viewport, `pageX` and `pageY` from the top left of the page, and whether it is `inViewport`.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
            }),
            &[],
        ),
        tool(
            "browser_resize",
            "Set the size of the page's viewport, in CSS pixels, by emulation, such as to test a narrow layout. The size stays through navigation and through the pane being resized; the pane shows the page at that size at its top left, and screenshots have that size. Give `reset` as true to follow the pane again. It sets a plain desktop viewport (pixel ratio of the pane, not mobile); see browser_emulate for devices.",
            &json!({
                "width": { "type": "integer", "minimum": 100, "maximum": 4000 },
                "height": { "type": "integer", "minimum": 100, "maximum": 4000 },
                "reset": { "type": "boolean" },
            }),
            &[],
        ),
        tool(
            "browser_cdp",
            "Send one raw Chrome DevTools Protocol command to the page and get its JSON result, cut at 20000 characters (a cut result is no longer valid JSON). Only a conservative allowlist is accepted so that it cannot leave the agent origin restriction or touch files: `DOM.*`, `CSS.*`, `Accessibility.*`, `DOMSnapshot.*` and `Performance.*` (except `DOM.setFileInputFiles` and `DOM.getFileInfo`), and these commands: `Runtime.evaluate`, `Runtime.callFunctionOn`, `Runtime.getProperties`, `Runtime.releaseObject`, `Runtime.releaseObjectGroup`, `Runtime.globalLexicalScopeNames`, `Runtime.getHeapUsage`, `Page.getLayoutMetrics`, `Page.getFrameTree`, `Page.captureScreenshot` (always PNG, returned as a picture), `Network.getResponseBody`, `Emulation.setEmulatedMedia`, `Emulation.setEmulatedVisionDeficiency`, `Emulation.setScrollbarsHidden`, and `Overlay.enable`, `Overlay.disable`, `Overlay.highlightNode`, `Overlay.highlightRect`, `Overlay.highlightQuad`, `Overlay.hideHighlight`. Everything else is refused with a message, including all navigation (`Page.navigate*`), `Target.*`, `Browser.*`, `IO.*`, `Fetch.*`, `Storage.*`, other `Network.*`, `Input.*` and the device-size and user-agent commands (use browser_emulate for those). Commands that change the page or its state (DOM and CSS edits, emulated media) are allowed and stay until undone. A command that takes over 30 seconds fails.",
            &json!({
                "method": { "type": "string", "description": "The command, such as DOM.getDocument or Runtime.evaluate." },
                "params": { "type": "object", "description": "The command's parameters, as the protocol defines them." },
            }),
            &["method"],
        ),
        tool(
            "browser_emulate",
            "Emulate a device for the page: give a `device` preset (`iphone-se`, `iphone-15`, `ipad`, `pixel-8`, `desktop-1280`, `desktop-1440`, `desktop-1920`; `iphone`, `pixel` and `desktop` are shortcuts), and/or `width` and `height` (100 to 4000 CSS pixels), `device_scale_factor` (up to 5), `mobile` (mobile layout, meta viewport honoured, touch events present) and `user_agent` (an empty string restores the browser's own). A `device` replaces all earlier emulation; other fields change just what they name. With no arguments it says what is emulated; `reset` true clears everything, the user agent too. The size and user agent stay through navigation, and the viewport stays through the pane being resized (the pane shows the page at that size at its top left, screenshots have that size). Limits: the engine is still Chromium, so a Safari user agent changes only the header and `navigator.userAgent`, not rendering or client hints (`navigator.userAgentData`); touch is emulated as touch event support, and clicks are still sent as mouse events.",
            &json!({
                "device": { "type": "string", "description": "A preset: iphone-se, iphone-15, ipad, pixel-8, desktop-1280, desktop-1440, desktop-1920, or iphone, pixel, desktop." },
                "width": { "type": "integer", "minimum": 100, "maximum": 4000 },
                "height": { "type": "integer", "minimum": 100, "maximum": 4000 },
                "device_scale_factor": { "type": "number", "exclusiveMinimum": 0, "maximum": 5 },
                "mobile": { "type": "boolean" },
                "user_agent": { "type": "string", "description": "The user agent string to send; empty restores the browser's own." },
                "reset": { "type": "boolean", "description": "Clear all emulation." },
            }),
            &[],
        ),
    ];
    for definition in &mut tools {
        definition["inputSchema"]["properties"]["tab"] = json!({
            "type": "integer",
            "description": "The tab to work on, by the number browser_tabs gives. Without it the tool works on the tab you last opened or selected with browser_tabs or named here, and otherwise on the tab the reader is looking at. A tab the reader locked refuses every tool that changes it; reading it with browser_snapshot, browser_screenshot, browser_console and browser_network is allowed.",
        });
    }
    tools.push(tool(
        "browser_tabs",
        "List, open, select and close the tabs of the browser pane. `list` gives every tab with its number, title and address and marks the one the reader is looking at, the one tools work on, the ones agents opened and the ones the reader locked. `open` makes a tab behind the one the reader looks at, on `url` or blank, marked as an agent's tab, and makes it the tab tools work on; agents may always work in their own tabs. `select` makes the tab named by `tab` the one tools work on without changing what the reader sees. `close` closes the tab named by `tab`. A tab the reader locked cannot be closed or changed by tools; open a tab of your own instead. At most 32 tabs are open.",
        &json!({
            "action": { "type": "string", "enum": ["list", "open", "select", "close"], "description": "What to do; list when omitted." },
            "url": { "type": "string", "description": "For open: the address to load, http or https; blank when omitted." },
            "tab": { "type": "integer", "description": "For select and close: the number of the tab." },
        }),
        &[],
    ));
    tools
}

/// The schema of one end of a drag: an element by `ref` or `selector`, or a point by `x` and `y`.
fn drag_end(target: &Value, description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "ref": target["ref"],
            "selector": target["selector"],
            "x": { "type": "number" },
            "y": { "type": "number" },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::definitions;

    /// The names of the tools, in order.
    fn names() -> Vec<String> {
        definitions()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// Every tool has a unique name, and the second round and the tabs end the list as one block.
    #[test]
    fn second_round_ends_the_list() {
        let names = names();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        let tail = &names[names.len() - 3..];
        assert_eq!(tail, ["browser_cdp", "browser_emulate", "browser_tabs"]);
    }

    /// The network and console tools take the new options.
    #[test]
    fn network_and_console_take_options() {
        let all = definitions();
        let properties = |name: &str| {
            let tool = all.iter().find(|tool| tool["name"] == name).unwrap();
            tool["inputSchema"]["properties"].clone()
        };
        for option in ["filter", "clear", "request"] {
            assert!(properties("browser_network").get(option).is_some());
        }
        for option in ["level", "clear"] {
            assert!(properties("browser_console").get(option).is_some());
        }
    }

    /// Every tool but browser_tabs takes an optional `tab`, and browser_tabs takes an action.
    #[test]
    fn tools_take_a_tab() {
        for tool in definitions() {
            let properties = &tool["inputSchema"]["properties"];
            assert!(properties.get("tab").is_some(), "{}", tool["name"]);
            assert!(
                !tool["inputSchema"]["required"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|name| name == "tab")
            );
        }
        let all = definitions();
        let tabs = all
            .iter()
            .find(|tool| tool["name"] == "browser_tabs")
            .unwrap();
        assert!(tabs["inputSchema"]["properties"].get("action").is_some());
    }
}
