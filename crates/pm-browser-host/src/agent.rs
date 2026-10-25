//! The tools agents call against a page: looking at it, reading it, and acting on it, all through the page's DevTools protocol.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use cef::*;
use serde_json::{Value, json};

use crate::bridge::Wish;
use crate::cdp::Cdp;
use crate::handler::Page;

/// The most lines the console and the network keep.
const KEPT: usize = 200;

/// The most characters of a tool's text that are given back.
const MOST_TEXT: usize = 20_000;

/// How long loading a page may take.
const LOADING: Duration = Duration::from_secs(20);

/// The name of the scripting world the tools read pages in.
const WORLD: &str = "pm-agent";

/// What a page wrote to its console and what it asked of the network, kept for the tools that report them.
#[derive(Default)]
pub(crate) struct Recorder {
    /// The console's lines, oldest first.
    console: Mutex<VecDeque<String>>,
    /// The requests, oldest first.
    network: Mutex<VecDeque<String>>,
    /// The request each id belongs to, until it is answered.
    requests: Mutex<HashMap<String, (String, String)>>,
}

/// Adds `line` to `lines`, forgetting the oldest when there are too many.
fn keep(lines: &Mutex<VecDeque<String>>, line: String) {
    if let Ok(mut lines) = lines.lock() {
        lines.push_back(line);
        while lines.len() > KEPT {
            lines.pop_front();
        }
    }
}

/// What a protocol value says, as text.
fn said(value: &Value) -> String {
    value["value"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| (!value["value"].is_null()).then(|| value["value"].to_string()))
        .or_else(|| value["description"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Recorder {
    /// Starts recording what the page of `host` reports, and has the engine report it.
    pub(crate) fn start(self: &Arc<Self>, cdp: &Cdp, host: &BrowserHost) {
        let held = self.clone();
        cdp.listen(move |method, params| match method {
            "Runtime.consoleAPICalled" => {
                let text = params["args"]
                    .as_array()
                    .map(|args| args.iter().map(said).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                keep(
                    &held.console,
                    format!("[{}] {text}", params["type"].as_str().unwrap_or("log")),
                );
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let text = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("an exception");
                keep(&held.console, format!("[error] {text}"));
            }
            "Log.entryAdded" => {
                let entry = &params["entry"];
                keep(
                    &held.console,
                    format!(
                        "[{}] {} {}",
                        entry["level"].as_str().unwrap_or("info"),
                        entry["text"].as_str().unwrap_or_default(),
                        entry["url"].as_str().unwrap_or_default()
                    ),
                );
            }
            "Network.requestWillBeSent" => {
                if let (Some(id), Some(url)) = (
                    params["requestId"].as_str(),
                    params["request"]["url"].as_str(),
                ) && let Ok(mut requests) = held.requests.lock()
                {
                    let method = params["request"]["method"].as_str().unwrap_or("GET");
                    requests.insert(id.to_owned(), (method.to_owned(), url.to_owned()));
                }
            }
            "Network.responseReceived" => {
                let known = params["requestId"]
                    .as_str()
                    .and_then(|id| held.requests.lock().ok()?.remove(id));
                if let Some((method, url)) = known {
                    keep(
                        &held.network,
                        format!("{method} {url} -> {}", params["response"]["status"]),
                    );
                }
            }
            "Network.loadingFailed" => {
                let known = params["requestId"]
                    .as_str()
                    .and_then(|id| held.requests.lock().ok()?.remove(id));
                if let Some((method, url)) = known {
                    keep(
                        &held.network,
                        format!(
                            "{method} {url} -> failed: {}",
                            params["errorText"].as_str().unwrap_or("unknown")
                        ),
                    );
                }
            }
            _ => {}
        });
        for method in [
            "Runtime.enable",
            "Network.enable",
            "Page.enable",
            "Log.enable",
        ] {
            cdp.call(host, method, &json!({}), |_| {});
        }
    }
}

/// The lines of `lines`, oldest first, or `none` when there are no lines.
fn lines_of(lines: &Mutex<VecDeque<String>>, none: &str) -> String {
    let lines = lines
        .lock()
        .map(|l| l.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if lines.is_empty() {
        none.to_owned()
    } else {
        lines.join("\n")
    }
}

/// `text` cut to at most `most` characters.
fn cut(mut text: String, most: usize) -> String {
    if let Some((end, _)) = text.char_indices().nth(most) {
        text.truncate(end);
        text.push_str("\n[cut]");
    }
    text
}

/// A JavaScript expression that finds the element the arguments name, or nothing when they name none.
fn target(arguments: &Value) -> Option<String> {
    if let Some(number) = arguments["ref"].as_u64() {
        return Some(format!("(window.__pmRefs || [])[{number}]"));
    }
    let selector = arguments["selector"].as_str()?;
    Some(format!(
        "document.querySelector({})",
        serde_json::to_string(selector).ok()?
    ))
}

/// Runs `expression` in the scripting context `context`, or the page's own when none, and hands the value to `then`; a failure ends the call.
fn evaluate(
    page: &Arc<Page>,
    call: u64,
    context: Option<i64>,
    expression: &str,
    then: impl FnOnce(&Arc<Page>, &Value) + Send + 'static,
) {
    let mut params =
        json!({ "expression": expression, "returnByValue": true, "awaitPromise": true });
    if let Some(context) = context {
        params["contextId"] = json!(context);
    }
    let held = page.clone();
    page.call("Runtime.evaluate", &params, move |reply| {
        if let Some(error) = reply["error"]["message"].as_str() {
            held.tool_done(call, false, error.to_owned(), None);
            return;
        }
        let result = &reply["result"];
        if result["exceptionDetails"].is_object() {
            let text = result["exceptionDetails"]["exception"]["description"]
                .as_str()
                .or_else(|| result["exceptionDetails"]["text"].as_str())
                .unwrap_or("the script failed");
            held.tool_done(call, false, text.to_owned(), None);
            return;
        }
        then(&held, &result["result"]["value"]);
    });
}

/// Hands `then` the scripting context the tools read the page in.
fn in_world(page: &Arc<Page>, call: u64, then: impl FnOnce(&Arc<Page>, i64) + Send + 'static) {
    let next = page.clone();
    page.call("Page.getFrameTree", &json!({}), move |reply| {
        let Some(frame) = reply["result"]["frameTree"]["frame"]["id"].as_str() else {
            next.tool_done(call, false, "the page has no frame yet".to_owned(), None);
            return;
        };
        let last = next.clone();
        next.call(
            "Page.createIsolatedWorld",
            &json!({ "frameId": frame, "worldName": WORLD, "grantUniveralAccess": false }),
            move |reply| match reply["result"]["executionContextId"].as_i64() {
                Some(context) => then(&last, context),
                None => last.tool_done(call, false, "the page could not be read".to_owned(), None),
            },
        );
    });
}

/// The JSON a script returned as text, parsed.
fn parsed(value: &Value) -> Value {
    value
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null)
}

/// Clicks at `x`, `y` in the page, as the reader would.
fn click_at(page: &Arc<Page>, call: u64, x: f64, y: f64) {
    let Some(host) = page.browser().and_then(|browser| browser.host()) else {
        page.tool_done(call, false, "the page is gone".to_owned(), None);
        return;
    };
    let event = MouseEvent {
        x: x.round() as i32,
        y: y.round() as i32,
        modifiers: 0,
    };
    host.send_mouse_move_event(Some(&event), false as _);
    host.send_mouse_click_event(Some(&event), MouseButtonType::LEFT, false as _, 1);
    host.send_mouse_click_event(Some(&event), MouseButtonType::LEFT, true as _, 1);
    page.tool_done(
        call,
        true,
        format!("Clicked at {}, {}.", event.x, event.y),
        None,
    );
}

/// Presses Enter in the page.
fn press_enter(page: &Arc<Page>) {
    let Some(host) = page.browser().and_then(|browser| browser.host()) else {
        return;
    };
    let event = |kind: KeyEventType, character: u16| KeyEvent {
        size: std::mem::size_of::<KeyEvent>(),
        type_: kind,
        modifiers: 0,
        windows_key_code: 13,
        native_key_code: 0,
        is_system_key: 0,
        character,
        unmodified_character: character,
        focus_on_editable_field: 0,
    };
    host.send_key_event(Some(&event(KeyEventType::RAWKEYDOWN, 0)));
    host.send_key_event(Some(&event(KeyEventType::CHAR, 13)));
    host.send_key_event(Some(&event(KeyEventType::KEYUP, 0)));
}

/// Opens `url` and ends the call when the page has loaded, or fails to, or takes too long.
fn navigate(page: &Arc<Page>, call: u64, url: &str, timers: &Sender<Wish>) {
    let done = Arc::new(AtomicBool::new(false));
    let (heard, flag) = (page.clone(), done.clone());
    page.cdp.listen(move |method, _| {
        if method == "Page.loadEventFired" && !flag.swap(true, Ordering::Relaxed) {
            heard.tool_done(call, true, "The page loaded.".to_owned(), None);
        }
    });
    let (held, flag) = (page.clone(), done.clone());
    page.call("Page.navigate", &json!({ "url": url }), move |reply| {
        if let Some(error) = reply["result"]["errorText"].as_str()
            && !flag.swap(true, Ordering::Relaxed)
        {
            held.tool_done(call, false, format!("The page did not load: {error}"), None);
        }
    });
    let (timers, id) = (timers.clone(), page.id());
    std::thread::spawn(move || {
        std::thread::sleep(LOADING);
        if !done.swap(true, Ordering::Relaxed) {
            let _ = timers.send(Wish::Late { page: id, call });
        }
    });
}

/// The script that lists what can be clicked or typed in, and what the page says.
const SNAPSHOT: &str = r#"(() => {
  const wanted = 'a,button,input,select,textarea,summary,[role=button],[role=link],[role=checkbox],[role=tab],[role=menuitem],[onclick],[contenteditable=""],[contenteditable=true],h1,h2,h3,h4,label';
  const seen = [];
  const lines = [];
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
  };
  for (const el of document.querySelectorAll(wanted)) {
    if (!visible(el)) continue;
    const text = (el.innerText || el.value || el.getAttribute('aria-label') || el.placeholder || el.title || '')
      .trim().replace(/\s+/g, ' ').slice(0, 80);
    const type = el.type ? ':' + el.type : '';
    seen.push(el);
    lines.push('[' + (seen.length - 1) + '] ' + el.tagName.toLowerCase() + type + (text ? ' "' + text + '"' : '') + (el.id ? ' #' + el.id : ''));
    if (seen.length >= 300) break;
  }
  window.__pmRefs = seen;
  return JSON.stringify({
    url: location.href,
    title: document.title,
    text: (document.body ? document.body.innerText : '').trim().slice(0, 3000),
    lines,
  });
})()"#;

/// Runs the tool `tool` against `page`, given `arguments` as JSON text, and ends the call `call` with what it came to.
pub(crate) fn run(page: &Arc<Page>, call: u64, tool: &str, arguments: &str, timers: &Sender<Wish>) {
    let arguments: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    match tool {
        "browser_navigate" => match arguments["url"].as_str() {
            Some(url) => navigate(page, call, url, timers),
            None => page.tool_done(call, false, "no url was given".to_owned(), None),
        },
        "browser_snapshot" => in_world(page, call, move |page, context| {
            evaluate(page, call, Some(context), SNAPSHOT, move |page, value| {
                let found = parsed(value);
                let lines: Vec<&str> = found["lines"]
                    .as_array()
                    .map(|lines| lines.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let text = format!(
                    "URL: {}\nTitle: {}\n\nElements:\n{}\n\nText:\n{}",
                    found["url"].as_str().unwrap_or_default(),
                    found["title"].as_str().unwrap_or_default(),
                    lines.join("\n"),
                    found["text"].as_str().unwrap_or_default(),
                );
                page.tool_done(call, true, cut(text, MOST_TEXT), None);
            });
        }),
        "browser_screenshot" => {
            let held = page.clone();
            page.call(
                "Page.captureScreenshot",
                &json!({ "format": "png" }),
                move |reply| {
                    let bytes = reply["result"]["data"].as_str().and_then(|data| {
                        base64::engine::general_purpose::STANDARD.decode(data).ok()
                    });
                    match bytes.and_then(|bytes| held.save_picture(&bytes)) {
                        Some(path) => {
                            held.tool_done(call, true, "Screenshot taken.".to_owned(), Some(path))
                        }
                        None => held.tool_done(
                            call,
                            false,
                            "no picture could be taken".to_owned(),
                            None,
                        ),
                    }
                },
            );
        }
        "browser_click" => {
            if let (Some(x), Some(y)) = (arguments["x"].as_f64(), arguments["y"].as_f64()) {
                click_at(page, call, x, y);
                return;
            }
            let Some(element) = target(&arguments) else {
                page.tool_done(
                    call,
                    false,
                    "name an element with ref or selector, or give x and y".to_owned(),
                    None,
                );
                return;
            };
            let script = format!(
                "(() => {{ const el = {element}; if (!el) return JSON.stringify({{ error: 'no such element; take a new browser_snapshot' }}); el.scrollIntoView({{ block: 'center', inline: 'center' }}); const r = el.getBoundingClientRect(); return JSON.stringify({{ x: r.x + r.width / 2, y: r.y + r.height / 2 }}); }})()"
            );
            in_world(page, call, move |page, context| {
                evaluate(page, call, Some(context), &script, move |page, value| {
                    let found = parsed(value);
                    match (found["x"].as_f64(), found["y"].as_f64()) {
                        (Some(x), Some(y)) => click_at(page, call, x, y),
                        _ => page.tool_done(
                            call,
                            false,
                            found["error"]
                                .as_str()
                                .unwrap_or("the element could not be found")
                                .to_owned(),
                            None,
                        ),
                    }
                });
            });
        }
        "browser_type" => {
            let (Some(text), Some(element)) = (
                arguments["text"].as_str().map(str::to_owned),
                target(&arguments),
            ) else {
                page.tool_done(
                    call,
                    false,
                    "give text, and an element by ref or selector".to_owned(),
                    None,
                );
                return;
            };
            let submit = arguments["submit"].as_bool().unwrap_or(false);
            let script = format!(
                "(() => {{ const el = {element}; if (!el) return JSON.stringify({{ error: 'no such element; take a new browser_snapshot' }}); el.scrollIntoView({{ block: 'center' }}); el.focus(); if (el.select) el.select(); return JSON.stringify({{ ok: true }}); }})()"
            );
            in_world(page, call, move |page, context| {
                evaluate(page, call, Some(context), &script, move |page, value| {
                    let found = parsed(value);
                    if let Some(error) = found["error"].as_str() {
                        page.tool_done(call, false, error.to_owned(), None);
                        return;
                    }
                    let held = page.clone();
                    page.call("Input.insertText", &json!({ "text": text }), move |_| {
                        if submit {
                            press_enter(&held);
                        }
                        held.tool_done(call, true, "Typed.".to_owned(), None);
                    });
                });
            });
        }
        "browser_evaluate" => match arguments["expression"].as_str() {
            Some(expression) => evaluate(page, call, None, expression, move |page, value| {
                page.tool_done(call, true, cut(value.to_string(), MOST_TEXT), None);
            }),
            None => page.tool_done(call, false, "no expression was given".to_owned(), None),
        },
        "browser_console" => page.tool_done(
            call,
            true,
            lines_of(&page.record.console, "The console is empty."),
            None,
        ),
        "browser_network" => page.tool_done(
            call,
            true,
            lines_of(&page.record.network, "No requests were made."),
            None,
        ),
        "browser_wait" => {
            let timeout = arguments["timeout_ms"]
                .as_u64()
                .unwrap_or(10_000)
                .min(30_000);
            let condition = if let Some(text) = arguments["text"].as_str() {
                format!(
                    "(document.body ? document.body.innerText : '').includes({})",
                    serde_json::to_string(text).unwrap_or_default()
                )
            } else if let Some(selector) = arguments["selector"].as_str() {
                format!(
                    "!!document.querySelector({})",
                    serde_json::to_string(selector).unwrap_or_default()
                )
            } else {
                "false".to_owned()
            };
            let script = format!(
                "new Promise((resolve) => {{ const start = Date.now(); const tick = () => {{ if ({condition}) resolve(true); else if (Date.now() - start > {timeout}) resolve(false); else setTimeout(tick, 100); }}; tick(); }})"
            );
            in_world(page, call, move |page, context| {
                evaluate(page, call, Some(context), &script, move |page, value| {
                    if value.as_bool() == Some(true) {
                        page.tool_done(call, true, "Found.".to_owned(), None);
                    } else {
                        page.tool_done(call, false, "Timed out.".to_owned(), None);
                    }
                });
            });
        }
        other => page.tool_done(call, false, format!("there is no tool {other}"), None),
    }
}
