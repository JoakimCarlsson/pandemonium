//! Picking an element of a page: a script that runs in a scripting world the page cannot see, and a binding only that world can call to report what was picked.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use pm_browser::PickedElement;
use serde_json::{Value, json};

use crate::handler::Page;

/// The script that, run in a page, lets the reader pick an element and reports it through the binding.
const SCRIPT: &str = include_str!("pick.js");

/// The name of the scripting world the script runs in.
const WORLD: &str = "pm-pick";

/// The name of the function that world reports through.
const BINDING: &str = "__pmPicked";

/// Turns pick mode on in `page`.
pub(crate) fn start(page: &Arc<Page>) {
    if !page.pick_listening.swap(true, Ordering::Relaxed) {
        let heard = page.clone();
        page.cdp.listen(move |method, params| {
            if method != "Runtime.bindingCalled" || params["name"] != BINDING {
                return;
            }
            let context = params["executionContextId"].as_i64();
            if context.is_none() || context != heard.pick_context() {
                return;
            }
            let Some(report) = params["payload"]
                .as_str()
                .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
            else {
                return;
            };
            heard.set_pick_context(None);
            let element = report
                .get("element")
                .and_then(|element| serde_json::from_value::<PickedElement>(element.clone()).ok());
            match element {
                Some(element) => heard.report_pick(Some(element)),
                None => heard.report_pick(None),
            }
        });
    }
    page.call("Runtime.enable", &json!({}), |_| {});
    page.call("Page.enable", &json!({}), |_| {});
    page.call(
        "Runtime.addBinding",
        &json!({ "name": BINDING, "executionContextName": WORLD }),
        |_| {},
    );
    let next = page.clone();
    page.call("Page.getFrameTree", &json!({}), move |reply| {
        let Some(frame) = reply["result"]["frameTree"]["frame"]["id"].as_str() else {
            return;
        };
        let last = next.clone();
        next.call(
            "Page.createIsolatedWorld",
            &json!({ "frameId": frame, "worldName": WORLD, "grantUniveralAccess": false }),
            move |reply| {
                let Some(context) = reply["result"]["executionContextId"].as_i64() else {
                    return;
                };
                last.set_pick_context(Some(context));
                last.call(
                    "Runtime.evaluate",
                    &json!({ "contextId": context, "expression": SCRIPT }),
                    |_| {},
                );
            },
        );
    });
}

/// Turns pick mode off in `page`.
pub(crate) fn stop(page: &Arc<Page>) {
    if let Some(context) = page.pick_context() {
        page.set_pick_context(None);
        page.call(
            "Runtime.evaluate",
            &json!({ "contextId": context, "expression": "window.__pmPick && window.__pmPick.stop(true);" }),
            |_| {},
        );
    }
}
