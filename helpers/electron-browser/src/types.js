'use strict';

/**
 * The messages the window and the helper send each other, one JSON object to a line, as
 * `crates/pm-browser/src/wire.rs` defines them. A message is serde's default form of a Rust enum:
 * a variant without fields is its name as a string, and any other is an object with one key, the
 * variant's name, holding the variant's fields.
 *
 * This file holds only type definitions, for the other files to import.
 */

/**
 * A mouse button.
 * @typedef {'Left' | 'Middle' | 'Right'} Button
 */

/**
 * What an edit request does to the page.
 * @typedef {'Cut' | 'Copy' | 'Paste' | 'SelectAll' | 'Undo' | 'Redo'} EditAction
 */

/**
 * How a zoom request changes the page's zoom.
 * @typedef {'In' | 'Out' | 'Reset'} ZoomAction
 */

/**
 * How the pointer looks over a page.
 * @typedef {'Arrow' | 'Hand' | 'Text' | 'Cross' | 'Wait' | 'Help' | 'ResizeHorizontal' | 'ResizeVertical' | 'Grab' | 'Grabbing' | 'Move' | 'NotAllowed' | 'ResizeNwse' | 'ResizeNesw'} Cursor
 */

/**
 * One press or release of a key.
 * @typedef {object} Key
 * @property {boolean} pressed Whether the key went down, not up.
 * @property {number} code The key as a Windows virtual-key code.
 * @property {string | null} text The text the key typed, if it typed any.
 * @property {number} modifiers The modifiers and buttons held, as bits.
 */

/**
 * What a tool came to, before the window's number for the call is added.
 * @typedef {object} ToolOutcome
 * @property {boolean} ok Whether the tool worked.
 * @property {string} text What it says: the result, or why it failed.
 * @property {string | null} image Where a picture the tool took was saved, when it took one.
 */

/**
 * One step of a recording of what agents did with the browser, as `browser_record` keeps it.
 * @typedef {object} RecordedStep
 * @property {string} folder The folder of the recording it belongs to.
 * @property {number} index Its number in the recording, from 1.
 * @property {string} tool The tool that was called, such as `browser_click`.
 * @property {string} arguments What the agent gave it, as JSON text, cut short.
 * @property {boolean} ok Whether the tool worked.
 * @property {string} text The start of what the tool answered.
 * @property {number} at When the tool ended, in milliseconds since the epoch.
 * @property {number} page The helper's number of the page it ran on.
 * @property {string} url The address of the page after it ran.
 * @property {string} picture The file of the screenshot taken after it ran, in the recording's folder, or empty when none could be taken.
 * @property {string} session The key of the agent session whose recording it belongs to, as the window names it; empty when the window named none.
 * @property {string} session_name What that session is called.
 */

/**
 * What running a tool came to; `step` is there while a recording is under way.
 * @typedef {ToolOutcome & { call: number, step?: RecordedStep }} ToolResult
 */

/**
 * An element of a page the reader picked.
 * @typedef {object} PickedElement
 * @property {string} selector A selector path that finds it, from the nearest id or a few levels up.
 * @property {string} tag Its tag name.
 * @property {[string, string][]} attributes Its attributes, name and value, in the order the page gave them.
 * @property {string} text Its text, on one line and cut short.
 * @property {[string, string][]} styles A few of its computed styles, name and value.
 * @property {number} width How wide its box is, in CSS pixels.
 * @property {number} height How tall its box is, in CSS pixels.
 * @property {string} component The name of the component that rendered it, when the page says so.
 * @property {[string, string][]} props That component's properties, name and value as written in markup: a string in quotes, anything else in braces.
 * @property {string} url The address of the page.
 * @property {string} title The title of the page.
 */

/**
 * What the reader adds to the prompt from a page.
 * @typedef {'Screenshot' | 'FullPage' | 'Area' | 'Console'} CaptureKind
 */

/**
 * One picture of a capture.
 * @typedef {object} CapturedPicture
 * @property {string} label What the picture shows, such as `Visible` or `Full page`.
 * @property {string} path Where the helper saved it, a file the window may read and remove.
 */

/**
 * What the reader asked to add to the prompt from a page, or why they got nothing.
 * @typedef {object} Capture
 * @property {CaptureKind} kind What was asked.
 * @property {CapturedPicture[]} pictures The pictures taken; none for the console and for an area that was not chosen.
 * @property {string[]} lines The console's lines, oldest first; only for the console.
 * @property {string} url The address of the page.
 * @property {string} title The title of the page.
 * @property {string} note Something the reader should know about what was taken, such as that an area was too large and the whole page was taken instead.
 * @property {string} error Why nothing could be taken; empty when something was, or when the reader chose no area.
 */

/**
 * The fields every request about a page has.
 * @typedef {object} PageBody
 * @property {number} page The page, by the number the window chose.
 */

/**
 * @typedef {PageBody & { url: string, width: number, height: number, scale: number, partition?: string }} OpenBody
 * Opens `url` in a new page of `width` by `height` logical pixels at display `scale`, in the profile `partition` names:
 * one starting `persist:` is kept on disk, any other in memory only; nothing or empty is the shared profile.
 */

/**
 * @typedef {PageBody & { of: number, width: number, height: number, scale: number }} OpenToolsBody
 * Opens the developer tools of the page `of` as a new page.
 */

/**
 * @typedef {PageBody & { url: string }} LoadBody
 * Navigates to `url`.
 */

/**
 * @typedef {PageBody & { width: number, height: number, scale: number }} ResizeBody
 * Gives the page a new size.
 */

/**
 * @typedef {PageBody & { focused: boolean }} FocusBody
 * Tells the page whether it has the keyboard.
 */

/**
 * @typedef {PageBody & { x: number, y: number, modifiers: number, left: boolean }} PointerMovedBody
 * Moves the pointer over the page, or out of it when `left`.
 */

/**
 * @typedef {PageBody & { x: number, y: number, button: Button, pressed: boolean, clicks: number, modifiers: number }} PointerButtonBody
 * Presses or releases a mouse button.
 */

/**
 * @typedef {PageBody & { x: number, y: number, dx: number, dy: number, modifiers: number, exact_dx?: number | null, exact_dy?: number | null, precise?: boolean }} ScrolledBody
 * Scrolls the page; a positive `dy` scrolls up. `exact_dx` and `exact_dy` give the distance with its fractions where the
 * window sends them, and `precise` says it came from a device that scrolls by the pixel, not from a wheel's notches.
 */

/**
 * @typedef {PageBody & { key: Key }} KeyBody
 * Presses or releases a key.
 */

/**
 * @typedef {PageBody & { text: string }} CommitTextBody
 * Types finished text, as an input method does.
 */

/**
 * @typedef {PageBody & { text: string, html?: string | null, files?: string[] }} PasteTextBody
 * Pastes what the window's clipboard held: its text, with its rich text as HTML and the paths of files when it held them.
 */

/**
 * @typedef {PageBody & { text: string }} ComposeBody
 * Shows text an input method is composing; empty text ends the composition without typing.
 */

/**
 * @typedef {PageBody & { call: number, tool: string, arguments: string, session?: string, session_name?: string }} ToolBody
 * Runs a tool an agent called against the page; `arguments` is JSON text. `session` is the key of the agent session that
 * made the call, which its recording is kept under, and `session_name` what that session is called; both are empty or
 * absent from windows that name no session.
 */

/**
 * @typedef {PageBody & { call: number, tool: string, arguments: string, ok: boolean, text: string, session?: string, session_name?: string }} RecordStepBody
 * Keeps a tool call the window answered itself, such as `browser_tabs`, in the recording of the session that made it,
 * with a screenshot of the page after it; the step comes back as `ToolDone` with the same `call`, and nothing comes back
 * when the session is not recorded. `page` is 0 when no page is to be pictured.
 */

/**
 * @typedef {PageBody & { action: EditAction }} EditBody
 * Cuts, copies or pastes in the page, or marks everything in it.
 */

/**
 * @typedef {PageBody & { on: boolean }} PickBody
 * Turns the mode on or off in which a click on the page picks the element under the pointer.
 */

/**
 * @typedef {PageBody & { text: string, forward: boolean, again: boolean }} FindBody
 * Finds `text` in the page; `again` steps to the next match of the text already searched for.
 */

/**
 * @typedef {PageBody & { action: ZoomAction }} ZoomBody
 * Changes the zoom of the page.
 */

/**
 * @typedef {PageBody & { kind: CaptureKind }} CaptureBody
 * Takes a picture of the page, or reads its console, for the reader to add to the prompt; the answer is a `Captured` report.
 */

/**
 * @typedef {PageBody & { on: boolean }} PickAreaBody
 * Turns the mode on or off in which the reader drags a rectangle over the page to take a picture of it.
 */

/**
 * What the window asks about a page that is not about opening or closing it, and is not input.
 * @typedef {{ Load: LoadBody }
 *   | { Back: PageBody }
 *   | { Forward: PageBody }
 *   | { Reload: PageBody }
 *   | { Resize: ResizeBody }
 *   | { Focus: FocusBody }
 *   | { Tool: ToolBody }
 *   | { RecordStep: RecordStepBody }
 *   | { Pick: PickBody }
 *   | { Find: FindBody }
 *   | { StopFind: PageBody }
 *   | { Zoom: ZoomBody }
 *   | { HardReload: PageBody }
 *   | { ClearData: PageBody }
 *   | { Capture: CaptureBody }
 *   | { PickArea: PickAreaBody }
 *   | { Stop: PageBody }
 *   | { Chrome: ChromeBody }} PageRequest
 */

/**
 * @typedef {PageBody & { action: Json }} ChromeBody
 * Something the reader does with the page from the pane around it, a `PageAction` of `crates/pm-browser/src/chrome.rs`, such as `{ AnswerDialog: { id, accept, text } }` or `'Print'`.
 */

/**
 * What the window asks of a page that is input: the pointer, the keyboard and the clipboard.
 * @typedef {{ PointerMoved: PointerMovedBody }
 *   | { PointerButton: PointerButtonBody }
 *   | { Scrolled: ScrolledBody }
 *   | { Key: KeyBody }
 *   | { CommitText: CommitTextBody }
 *   | { Edit: EditBody }
 *   | { PasteText: PasteTextBody }
 *   | { Compose: ComposeBody }} InputRequest
 */

/**
 * @typedef {{ restrict: boolean, origins: string[] }} AgentOriginsBody
 * Sets which origins agents' tools may take a page to: when `restrict`, only local addresses and `origins`.
 */

/**
 * @typedef {{ block_trackers: boolean, save_passwords: boolean }} PrivacyBody
 * Sets whether ads and trackers are blocked and whether logins are offered to be kept and filled in.
 */

/**
 * @typedef {PageBody & { id: number, allow: boolean }} PermissionAnswerBody
 * The reader's answer to the permission question `id`.
 */

/**
 * @typedef {PageBody & { proceed: boolean }} CertificateChoiceBody
 * The reader's choice on a certificate error: load anyway, or go back.
 */

/**
 * @typedef {PageBody & { html: string, title: string, width: number, height: number, scale: number }} OpenHtmlBody
 * Opens the document `html` in a new page, sealed from the network, called `title` while the document names nothing.
 */

/**
 * @typedef {{ ticket: number, html: string, width: number, height: number, path: string, scale?: number }} RenderHtmlBody
 * Renders the document `html`, sealed from the network, at a viewport of `width` by `height` CSS pixels and display `scale` (1 when missing) into the PNG file `path`; the answer is an `HtmlRendered` report with the same `ticket`.
 */

/**
 * @typedef {PageBody & { popup: number, width: number, height: number, scale: number }} AdoptPopupBody
 * Makes the window `popup` a page opened with `window.open` the new page `page`, of `width` by `height` logical pixels at display `scale`.
 */

/**
 * What the window asks of the helper.
 * @typedef {'Shutdown'
 *   | { Open: OpenBody }
 *   | { OpenTools: OpenToolsBody }
 *   | { Close: PageBody }
 *   | PageRequest
 *   | InputRequest
 *   | { AgentOrigins: AgentOriginsBody }
 *   | { PermissionAnswer: PermissionAnswerBody }
 *   | { CertificateChoice: CertificateChoiceBody }
 *   | { OpenHtml: OpenHtmlBody }
 *   | { AdoptPopup: AdoptPopupBody }
 *   | { DropPopup: { popup: number } }
 *   | { RenderHtml: RenderHtmlBody }
 *   | { Privacy: PrivacyBody }} Request
 */

/**
 * What is wrong with the certificate of a site.
 * @typedef {object} CertificateInfo
 * @property {string} url The address that was being opened.
 * @property {string} error The engine's name for what is wrong, such as `ERR_CERT_AUTHORITY_INVALID`.
 * @property {string} subject Who the certificate was issued to.
 * @property {string} issuer Who issued it.
 * @property {string} fingerprint Its SHA-256 fingerprint.
 * @property {string} expires When it expires, or an empty string when that is not known.
 */

/**
 * How a download went.
 * @typedef {'Started' | 'Finished' | 'Failed'} DownloadState
 */

/**
 * What the helper reports to the window.
 * @typedef {'Ready'
 *   | { Surface: { page: number, width: number, height: number, paths: [string, string] } }
 *   | { Painted: { page: number, buffer: number, generation: number, dirty?: [number, number, number, number] } }
 *   | { Picked: { page: number, element: PickedElement | null } }
 *   | { ToolDone: { page: number, result: ToolResult } }
 *   | { Title: { page: number, title: string } }
 *   | { Address: { page: number, url: string } }
 *   | { Cursor: { page: number, cursor: Cursor } }
 *   | { Found: { page: number, active: number, total: number } }
 *   | { Zoomed: { page: number, factor: number } }
 *   | { PermissionAsked: { page: number, id: number, origin: string, permission: string } }
 *   | { PermissionCancelled: { page: number, id: number } }
 *   | { CertificateError: { page: number, info: CertificateInfo } }
 *   | { CertificateCleared: { page: number } }
 *   | { Download: { page: number, name: string, state: DownloadState } }
 *   | { Captured: { page: number, capture: Capture } }
 *   | { OpenRequested: { page: number, url: string, agent: boolean } }
 *   | { HtmlRendered: { ticket: number, path: string, error: string } }
 *   | { Features: { features: string[] } }
 *   | { Loading: { page: number, loading: boolean, progress: number } }
 *   | { LoadFailed: { page: number, url: string, code: number, name: string } }
 *   | { Crashed: { page: number, reason: string } }
 *   | { Responsive: { page: number, responsive: boolean } }
 *   | { History: { page: number, back: boolean, forward: boolean } }
 *   | { PopupOpened: { page: number, popup: number, url: string, agent: boolean } }
 *   | { PageClosed: { page: number } }
 *   | { Copied: PageBody & { text: string, html: string } }
 *   | { Caret: PageBody & { x: number, y: number, width: number, height: number } }
 *   | { Chrome: PageBody & { report: Json } }} Event
 */

/**
 * What a page reports to the window: an event, said through the function the helper gives it.
 * @callback Send
 * @param {Event} event The report.
 * @returns {void}
 */

/**
 * The values a DevTools protocol command takes and gives back.
 * @typedef {Record<string, any>} Json
 */

module.exports = {};
