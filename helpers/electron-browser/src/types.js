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
 * @typedef {'Cut' | 'Copy' | 'Paste' | 'SelectAll'} EditAction
 */

/**
 * How a zoom request changes the page's zoom.
 * @typedef {'In' | 'Out' | 'Reset'} ZoomAction
 */

/**
 * How the pointer looks over a page.
 * @typedef {'Arrow' | 'Hand' | 'Text' | 'Cross' | 'Wait' | 'Help' | 'ResizeHorizontal' | 'ResizeVertical'} Cursor
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
 * What running a tool came to.
 * @typedef {ToolOutcome & { call: number }} ToolResult
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
 * @typedef {PageBody & { url: string, width: number, height: number, scale: number }} OpenBody
 * Opens `url` in a new page of `width` by `height` logical pixels at display `scale`.
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
 * @typedef {PageBody & { x: number, y: number, dx: number, dy: number, modifiers: number }} ScrolledBody
 * Scrolls the page; a positive `dy` scrolls up.
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
 * @typedef {PageBody & { call: number, tool: string, arguments: string }} ToolBody
 * Runs a tool an agent called against the page; `arguments` is JSON text.
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
 *   | { Pick: PickBody }
 *   | { Find: FindBody }
 *   | { StopFind: PageBody }
 *   | { Zoom: ZoomBody }
 *   | { HardReload: PageBody }
 *   | { ClearData: PageBody }
 *   | { Capture: CaptureBody }
 *   | { PickArea: PickAreaBody }} PageRequest
 */

/**
 * What the window asks of a page that is input: the pointer, the keyboard and the clipboard.
 * @typedef {{ PointerMoved: PointerMovedBody }
 *   | { PointerButton: PointerButtonBody }
 *   | { Scrolled: ScrolledBody }
 *   | { Key: KeyBody }
 *   | { CommitText: CommitTextBody }
 *   | { Edit: EditBody }} InputRequest
 */

/**
 * @typedef {{ restrict: boolean, origins: string[] }} AgentOriginsBody
 * Sets which origins agents' tools may take a page to: when `restrict`, only local addresses and `origins`.
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
 * What the window asks of the helper.
 * @typedef {'Shutdown'
 *   | { Open: OpenBody }
 *   | { OpenTools: OpenToolsBody }
 *   | { Close: PageBody }
 *   | PageRequest
 *   | InputRequest
 *   | { AgentOrigins: AgentOriginsBody }
 *   | { PermissionAnswer: PermissionAnswerBody }
 *   | { CertificateChoice: CertificateChoiceBody }} Request
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
 *   | { Painted: { page: number, buffer: number, generation: number } }
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
 *   | { Captured: { page: number, capture: Capture } }} Event
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
