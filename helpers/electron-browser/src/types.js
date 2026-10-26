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
 * @property {string} selector A selector that finds it, from the nearest id or a few levels up.
 * @property {string} tag Its tag name.
 * @property {string} id Its id, when it has one.
 * @property {string[]} classes Its classes.
 * @property {string} text Its text, cut short.
 * @property {string} html Its HTML, cut short.
 * @property {number} x Where its box is in the page, across, in CSS pixels.
 * @property {number} y Where its box is in the page, down, in CSS pixels.
 * @property {number} width How wide its box is, in CSS pixels.
 * @property {number} height How tall its box is, in CSS pixels.
 * @property {number} scroll_x How far the page was scrolled across when it was picked.
 * @property {number} scroll_y How far the page was scrolled down when it was picked.
 * @property {string | null} screenshot Where a picture of the element was saved; the helper takes none.
 * @property {[string, string][]} styles A few of its computed styles, name and value.
 * @property {string} url The address of the page.
 * @property {string} title The title of the page.
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
 * What the window asks about a page that is not about opening or closing it, and is not input.
 * @typedef {{ Load: LoadBody }
 *   | { Back: PageBody }
 *   | { Forward: PageBody }
 *   | { Reload: PageBody }
 *   | { Resize: ResizeBody }
 *   | { Focus: FocusBody }
 *   | { Tool: ToolBody }
 *   | { Pick: PickBody }} PageRequest
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
 * What the window asks of the helper.
 * @typedef {'Shutdown'
 *   | { Open: OpenBody }
 *   | { OpenTools: OpenToolsBody }
 *   | { Close: PageBody }
 *   | PageRequest
 *   | InputRequest} Request
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
 *   | { Cursor: { page: number, cursor: Cursor } }} Event
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
