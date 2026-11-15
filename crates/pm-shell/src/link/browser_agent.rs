//! The tools agents browse with: tells the server whether this window can run them, runs each call on the tabs of the session whose agent made it, and answers with what the page said.

use pm_protocol::Request;
use serde_json::Value;

use super::{Ask, Link};
use crate::browser::Browser;
use crate::browser_scope::NO_SCOPE;
use crate::browser_tools::{
    agent_may_open, changes_page, locked_message, origin_allowed, tabs_text,
};

/// What a tool call came to when the window answers it itself.
type Answer = Result<String, String>;

impl Link {
    /// Tells the server whether this window can run the tools of agents when that changed, and sends the answers of the calls that ended.
    pub(super) fn browser_agent(&mut self) {
        if let Some(available) = self.web.announcement(&self.settings.window.general) {
            self.ask(Request::BrowserAvailable { available }, Ask::Nothing);
            for host in self.remotes.hosts() {
                self.ask_remote(&host, Request::BrowserAvailable { available }, Ask::Nothing);
            }
        }
        for result in self.web.take_results() {
            let image = result
                .image
                .as_deref()
                .and_then(|path| {
                    let bytes = std::fs::read(path).ok();
                    let _ = std::fs::remove_file(path);
                    bytes
                })
                .unwrap_or_default();
            let answer = if result.ok {
                Ok(result.text)
            } else {
                Err(result.text)
            };
            self.browser_answer(result.call, answer, image);
        }
    }

    /// Tells the server what the call `call` came to.
    fn browser_answer(&mut self, call: u64, answer: Answer, image: Vec<u8>) {
        let (ok, text) = match answer {
            Ok(text) => (true, text),
            Err(text) => (false, text),
        };
        self.ask(
            Request::BrowserResult {
                call,
                ok,
                text,
                image,
            },
            Ask::Nothing,
        );
    }

    /// Runs the tool `tool` an agent of the session `session` called as `call`, with `arguments` as JSON text, on that session's tabs; answers at once when the window can, and otherwise once the page does.
    pub(super) fn browser_call(&mut self, call: u64, session: u64, tool: &str, arguments: &str) {
        let general = &self.settings.window.general;
        if !crate::web::Web::ready(general) {
            let answer = Err("The embedded browser is not usable here.".to_owned());
            return self.browser_answer(call, answer, Vec::new());
        }
        let scope = if session == NO_SCOPE {
            self.workspace.shelf.scope
        } else {
            session
        };
        let shown = self.workspace.shelf.scope;
        let host = self
            .sessions
            .iter()
            .find(|info| info.session == scope)
            .and_then(|info| pm_protocol::host_of(&info.directory))
            .map(|(host, _)| host.to_owned());
        let workspace = &mut self.workspace;
        workspace.shelf.swap(&mut workspace.browser, scope);
        let answer = self.browser_tool(call, host, tool, arguments);
        let workspace = &mut self.workspace;
        workspace.shelf.swap(&mut workspace.browser, shown);
        if let Some(answer) = answer {
            self.browser_answer(call, answer, Vec::new());
        }
        self.run_web();
    }

    /// Starts the tool on the browser the shelf now shows; the answer when the window gives it itself, nothing when the page will.
    ///
    /// A tool works on the tab its `tab` argument names, which then is the one tools work on when they name none; with no such argument, on the tab an agent chose or opened, and else on the one shown. A tool that changes a page is refused on a tab the reader locked.
    fn browser_tool(
        &mut self,
        call: u64,
        host: Option<String>,
        tool: &str,
        arguments: &str,
    ) -> Option<Answer> {
        let mut arguments: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
        if tool == "browser_tabs" {
            return Some(self.browser_tabs_tool(host, &arguments));
        }
        let named = match arguments.get("tab") {
            None | Some(Value::Null) => None,
            Some(value) => match value.as_u64().and_then(|id| u32::try_from(id).ok()) {
                Some(id) => Some(id),
                None => {
                    return Some(Err(
                        "tab must be the number of a tab, as browser_tabs lists them".to_owned(),
                    ));
                }
            },
        };
        if let Some(object) = arguments.as_object_mut() {
            object.remove("tab");
        }
        if tool == "browser_navigate" {
            let Some(url) = arguments["url"].as_str().map(str::to_owned) else {
                return Some(Err("no url was given".to_owned()));
            };
            if !agent_may_open(&url) {
                return Some(Err(
                    "Only http and https addresses can be opened.".to_owned()
                ));
            }
            match self.web.resolve_for_tool(&url, host.as_deref()) {
                Ok(resolved) => arguments["url"] = Value::String(resolved),
                Err(message) => return Some(Err(message)),
            }
        }
        let target = match (named, self.workspace.browser.as_ref()) {
            (Some(id), Some(browser)) if browser.tab(id).is_some() => id,
            (Some(id), _) => {
                return Some(Err(format!(
                    "There is no tab {id}; browser_tabs lists the tabs there are."
                )));
            }
            (None, Some(browser)) => browser.agent_target(),
            (None, None) if tool == "browser_navigate" => {
                let mut browser = Browser::new("about:blank", host);
                browser.tabs[0].agent = true;
                let id = browser.active_id();
                browser.agent_tab = Some(id);
                self.workspace.browser = Some(browser);
                id
            }
            (None, None) => {
                return Some(Err(
                    "No page is open in the browser; use browser_navigate first.".to_owned(),
                ));
            }
        };
        let Some(browser) = self.workspace.browser.as_mut() else {
            return Some(Err("No page is open in the browser.".to_owned()));
        };
        if browser.tab(target).is_some_and(|tab| tab.locked) && changes_page(tool) {
            return Some(Err(locked_message(tool, target)));
        }
        if named.is_some() {
            browser.agent_tab = Some(target);
        }
        let general = &self.settings.window.general;
        self.web.carry_out(&mut self.workspace.browser, general);
        if self.web.tool(target, call, tool, &arguments.to_string()) {
            None
        } else {
            Some(Err(format!(
                "No page is open in tab {target}; use browser_navigate with tab {target} first."
            )))
        }
    }

    /// Runs `browser_tabs`, which the window answers itself: `list`, `open`, `select` and `close`.
    fn browser_tabs_tool(&mut self, host: Option<String>, arguments: &Value) -> Answer {
        let tab = arguments["tab"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok());
        let general = &self.settings.window.general;
        let (restricted, origins) = (
            general.restrict_agent_browsing,
            general.agent_origins.clone(),
        );
        let listed = tabs_text(self.workspace.browser.as_ref());
        match arguments["action"].as_str().unwrap_or("list") {
            "list" => Ok(listed),
            "open" => {
                let url = arguments["url"].as_str().unwrap_or("about:blank").trim();
                if !agent_may_open(url) {
                    return Err("Only http and https addresses can be opened.".to_owned());
                }
                if !origin_allowed(url, restricted, &origins) {
                    return Err(format!(
                        "Blocked: {url} is not an origin agents may open. Agent browsing is restricted to localhost, 127.0.0.1, [::1] and the origins the reader added in Settings under Browser."
                    ));
                }
                self.web.resolve_for_tool(url, host.as_deref())?;
                let id = match self.workspace.browser.as_mut() {
                    Some(browser) => browser.open_tab(url, host, true, false),
                    None => {
                        let mut browser = Browser::new(url, host);
                        browser.tabs[0].agent = true;
                        let id = browser.active_id();
                        browser.agent_tab = Some(id);
                        self.workspace.browser = Some(browser);
                        Some(id)
                    }
                };
                let general = &self.settings.window.general;
                self.web.carry_out(&mut self.workspace.browser, general);
                match id {
                    Some(id) => Ok(format!(
                        "Opened tab {id} at {url}. It is the tab the tools work on from now on, and the reader sees it in the strip, marked as yours. Give tab {id} to a tool to name it."
                    )),
                    None => Err("The browser holds too many tabs; close one first.".to_owned()),
                }
            }
            action @ ("select" | "close") => {
                let Some(browser) = self.workspace.browser.as_mut() else {
                    return Err("No tabs are open.".to_owned());
                };
                let Some(id) = tab.filter(|id| browser.tab(*id).is_some()) else {
                    return Err(format!(
                        "Give tab as the number of a tab; the tabs are:\n{listed}"
                    ));
                };
                let locked = browser.tab(id).is_some_and(|held| held.locked);
                if action == "close" {
                    if locked {
                        return Err(locked_message("browser_tabs close", id));
                    }
                    if let Some(index) = browser.tab_index(id)
                        && browser.close_tab(index)
                    {
                        self.workspace.browser = None;
                    }
                    return Ok(format!("Closed tab {id}."));
                }
                browser.agent_tab = Some(id);
                Ok(format!(
                    "Tools now work on tab {id}; what the reader sees is unchanged.{}",
                    if locked {
                        " It is locked by the reader, so tools that change it are refused; reading it is allowed."
                    } else {
                        ""
                    }
                ))
            }
            other => Err(format!(
                "there is no action {other}; use list, open, select or close"
            )),
        }
    }
}
