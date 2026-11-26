# The embedded browser

The browser pane shows web pages beside an agent's work, such as the dev
server it is building. It is opt-in: Settings, under Browser, turns it on and
downloads the engine once, about 120 MB.

## Parts

- `pm-browser` is the client crate the window links. An `Engine` starts the
  helper, a `View` is one page, and `install` downloads and unpacks the helper.
  It knows nothing of windows, GPUs or Chromium.
- `helpers/electron-browser` is the helper, `pandemonium-browser`: an Electron
  app. Electron carries Chromium with the H.264 and AAC decoders, so video that
  Chromium alone cannot play works in the pane.
- `pm-components` draws the pane, and `crates/pandemonium/src/app/browser.rs`
  carries what the pane asks for to the helper.
- `pm-server` hosts the MCP endpoint that gives agents the `browser_*` tools;
  the tools run in the helper of the window that announced the browser, on the
  tabs of the session whose agent called them. See
  [protocol.md](protocol.md#browser-tools) and
  [Tabs belong to a session](#tabs-belong-to-a-session).

## How the window and the helper talk

The window listens on a loopback port and starts the helper with `--control
<host:port> --surfaces <folder> --profile <folder>`, passing a random token in
`PANDEMONIUM_BROWSER_TOKEN`. The helper connects and sends the token as its
first line; the window ignores any connection that does not. Loopback TCP is
used on every system, since Windows has no Unix sockets in the standard
library. Requests and reports are one JSON object to a line; `crates/pm-browser/src/wire.rs` defines them.

Each page paints into two files in the surfaces folder, written in turn. A
file starts with a counter that is odd while the helper fills it and even
otherwise; the window copies the pixels out only when it sees the same even
number before and after, and never from a file whose counter is still 0,
which was not filled yet. A `Surface` report names the files and a `Painted`
report says which is newest.

The four bytes after the counter are `BGRA` when the pixels are stored as the
engine paints them; the helper writes them so and does no work per pixel, and
neither does the window: it hands them to the renderer as BGRA
(`rustle::PixelFormat::Bgra8`), which keeps them in a BGRA texture. A file
without the tag holds RGBA and is drawn as such. The helper writes into a file
only the rows that changed since that file was last written: the paint's dirty
rectangle joined with the one of the paint before, since the files take turns
(`dirtyRows` in `src/surface.js`).

Each `Painted` report also carries the paint's dirty rectangle in device
pixels (`dirty`, left, top, width and height; `dirtyRect` in
`src/surface.js`), and the window remembers the rectangles of the last 32
paints (`crates/pm-browser/src/damage.rs`); the first paint into new files
counts as changing everything, and a report without the field as well. The
window copies into the pixels of the picture before last once the renderer no
longer holds them, and copies only the rectangle the paints since that picture
changed (`SurfaceReader::read_region_into`), or everything when one of those
paints is not remembered. It hands the pixels to the renderer without another
copy, with the rectangle that changed since the picture handed out before
(`Frame::changed`), and the renderer writes only that rectangle into the
page's texture while the texture still holds that picture
(`ImageData::changed_since`); otherwise it writes the whole picture.

When the size changes, the helper makes new files and announces them before
any paint into them. It keeps the old files for two seconds before removing
them, so a window still mapping them is not left with missing files, and
retries later where the system refuses to remove a mapped file. The window
swaps in the new files and their size under one lock; until a paint into them
arrives it keeps showing the last picture at the old size, and if it cannot
map them it shows that picture rather than reading the wrong files.

The window reads reports as bytes up to each newline and the helper splits
requests the same way (`src/lines.js`), so a character whose bytes arrive in
two chunks is never broken.

### Display scale

`Open`, `OpenTools`, `OpenHtml`, `RenderHtml` and `Resize` carry the pane's
display `scale` (device pixels per logical pixel), kept between 1 and 4
(`displayScale` in `src/surface.js`). A page on a 2x display paints twice its
logical size in each direction. The surface files hold device pixels (`pixels`
in `src/surface.js` takes the bitmap at the image's sharpest scale), and the
window draws the frame cropped to the pane's logical size, so it shows sharp.
The requests of the window stay in logical pixels.

The engine fixes an offscreen window's device scale when it makes the window and
has no way to change it later. Under Wayland it ignores any scale but 1 while
Chromium's `WaylandFractionalScaleV1` feature is on, so the helper turns that
feature off (`main.js`); the windows are offscreen and never shown, so the
desktop's fractional scaling of real windows does not matter to them.
Every window paints at 1 (`ENGINE_SCALE` in `src/surface.js`), except that of a
mobile viewport (below), and is sized
in device pixels, the pane's logical size times its scale (`Page#fitWindow`;
the size is set again once the window exists, since X11 shrinks a new window
to the screen),
and the page's zoom carries the scale: the contents' zoom is the pane's scale
times the reader's zoom (`Page#carryScale`, `rendering` in `src/emulation.js`),
so the page lays out at the logical size and reports and renders at a pixel
ratio of the scale. The zoom, unlike an emulated pixel ratio, reaches
cross-origin frames, which run in processes of their own:
`Emulation.setDeviceMetricsOverride` changes the top frame alone, and a frame's
target refuses it ("can only be executed on top-level targets"), so with it
such a frame reports a pixel ratio of 1 and is painted blurry. The contents'
zoom mode is `isolated`, so the zoom stays through navigation and no other page
of the site takes it; the helper puts it back after a navigation when the
engine dropped it, as it does for a zoom set before the first document comes up
(`Page#holdZoom`).

The engine holds the contents' zoom to 25%–500%. When the pane's scale times
the reader's zoom goes past that, as at scale 2 above 250%, the zoom stays at
the limit and an override that follows the window (width and height 0) carries
the rest: its `deviceScaleFactor` and its `scale` are both what is left over,
so the top frame lays out at the window's size over the scale times the
reader's zoom and reports that times its pixel ratio. Cross-origin frames get
the zoom alone, so at scale 2 and 500% they lay out right and render at a
pixel ratio of 5, scaled up by 2. The reader's zoom, as `Zoom`, `SetZoom` and
`Zoomed` carry it, is the zoom the reader chose (`Page#readerZoom`), through
the whole range of 25% to 500% at any scale.

The developer tools and sealed pages render at the pane's scale through
`Emulation.setDeviceMetricsOverride` with width and height 0 (the size follows
the window) and `deviceScaleFactor` and `scale` both the pane's scale
(`paneMetrics` in `src/emulation.js`), through a debugger the helper attaches
once the scale is above 1. When a `Resize` brings another scale, as when the
window moves to another display, the window is sized again and the zoom or the
override set again (`Page#applyScale`), so frames come out at `width * scale`
by `height * scale`, as sharp as a page opened at that scale, and the page keeps
its state. `RenderHtml` sizes and renders its window the same way.

The window is in device pixels, so the helper turns every point it sends into
them: pointer and wheel positions of the window and the points of `InspectAt`
and `CopyImageAt` are multiplied by the scale (`toWindow` in `src/input.js`),
and the point of a right click is divided by it. The points of agents' clicks,
hovers and drags are CSS pixels of the top frame, as the boxes of elements are,
so they are multiplied by the reader's zoom as well (`windowPoint` in
`src/agent.js`). Wheel distances stay as they are, since the engine scrolls
them in CSS pixels.

An agent's viewport (`browser_resize`, `browser_emulate`) sizes the window to
the viewport's size times the pane's scale, and the same split renders it
(`rendering`, `Page#emulateViewport`): the zoom carries the viewport's pixel
ratio, or the pane's, times the reader's zoom as far as the engine takes it, and
the override follows the window with what is left as its `deviceScaleFactor`,
the layout's share as its `scale` and the viewport's size as the screen's. A
viewport with a pixel ratio of its own that is not mobile therefore reaches
cross-origin frames: at scale 2, ratio 3 is a zoom of 3 with an override of
ratio 1 and `scale` 2/3, and the top frame and its cross-origin frames both
report 3 and lay out at the viewport's size. Points of the window reach the top
frame's target in CSS pixels times the override's `scale`, which is the
window's pixels over the contents' zoom (`Router#inside`, `Page#topUnits`).

A mobile viewport cannot use the zoom: while the engine emulates a phone it
ignores the zoom in the top frame but still zooms cross-origin frames by it,
which then lay out too large. Its zoom stays 1 and the override carries its
pixel ratio times the reader's zoom. Cross-origin frames then render at the
window's own device scale: a frame's target refuses the override, and
`enableDeviceEmulation` of Electron reaches the top frame alone. So when an
agent turns on a mobile viewport whose ratio is not 1, the page moves into a
new window painted at that ratio (`engineScale` in `src/emulation.js`,
`Page#rebuild`), and back into one painted at 1 when the viewport stops being
mobile or is reset. The old window is closed without `PageClosed`, the new one
is watched as the old one was, and the page's history is handed over
(`navigationHistory.restore`), which loads its document again: what the
document held in memory, such as typed text, is lost, as the console and
network the recorder kept are. A popup an adopted tab shows cannot move, since
its window belongs to its opener, so its cross-origin frames keep a ratio of 1
under a mobile viewport.

The moved window is the device size over the ratio, rounded up
(`Page#windowSize`); what it paints past the device size is cut off
(`Page#paint`). Its override gives the layout size itself, the viewport's size
over the reader's zoom, with `dontSetVisibleSize` so the engine keeps the
window's size, and `scale` the pane's scale times the reader's zoom over the
window's ratio, which shrinks the layout into it: at scale 2, a 300 by 200
viewport of ratio 3 is a window of 200 by 134 painted at 3, of which 600 by 400
is kept, and the top frame and its cross-origin frames both report 3. A
reader's zoom does not move the page again: the cross-origin frames keep the
ratio and are scaled up by the zoom. Points
of the window reach the top frame's target over the window's ratio as well
(`Page#topUnits`), and Electron's own input events, menus and the points of
`InspectAt` and `CopyImageAt` are in the window's pixels, the pane's scale over
that ratio (`Page#inputScale`).

Right after the token the helper sends `Features { features }`, the names of
what it can do beyond what every helper does, and then `Ready`. The window reads
both before it hands the engine out (`Engine::start` waits up to ten seconds),
so it knows at once what the helper supports. A helper that sends only `Ready`,
as helpers released before a feature existed do, has none of them, and the
window never sends it a request of a feature it did not announce: such a helper
would ignore the request and leave the window waiting. Today the one feature is
`sealed-pages` (`SEALED_PAGES` in `wire.rs`, `src/features.js` in the helper),
for `OpenHtml` and `RenderHtml`. `partitions` (`PARTITIONS`) says the helper
opens a page in the profile the `partition` of `Open` names (see
[Private tabs and profiles](#private-tabs-and-profiles)).

`OpenRequested` reports that a page asked for a new window, so that the window
opens a tab for it, and `PopupOpened`, `AdoptPopup` and `DropPopup` carry the
real popups that keep `window.opener` (see [Tabs](#tabs)). `Loading`,
`LoadFailed`, `Crashed`, `Responsive`, `History` and `PageClosed` report where a
page is in its life, and `Stop` stops a load (see
[Loading, failed loads and crashes](#loading-failed-loads-and-crashes)).

The helper runs each page as an offscreen Electron window and replays the
window's pointer, scroll and key requests as input events. The page counts as active only while its hidden window
is focused, so `Focus` focuses and blurs that window: without it a field shows no
caret or focus ring and a selection paints grey. Cut, copy, paste, select all,
undo (shortcut key with Z) and redo (with Shift+Z, or Y outside macOS), pressed
with the platform's shortcut key (Command on macOS, Control elsewhere), run as
the page's own edit commands, since Chromium binds those keys in a browser shell
that an offscreen page lacks (`shortcutEdit` in
`helpers/electron-browser/src/input.js`). The hidden window never reaches the
system clipboard, so the window owns it: on copy or cut, from a key or the menu,
the helper reads what is marked in the frame that holds the keyboard (a text
field's selection, otherwise the document's selection as text and HTML; never a
password field), reports it as `Copied`, and for a cut then deletes it; the
window puts the text on its clipboard. On paste, from the shortcut or the menu,
the window reads its clipboard and sends it as `PasteText`, which replaces
the selection: the text, with the rich text as HTML (`Clipboard::html` in
rustle) and the files the clipboard holds, or a picture on it saved as a PNG in
the `pasted` folder of the data directory. Plain text is typed as before. A
paste with HTML or files is first offered to the frame that holds the keyboard
as a paste event of its own (`src/paste.js`), its `clipboardData` holding the
text, the HTML and the files (at most 25 MB of them), so rich editors keep the
formatting and pages that take pasted pictures, such as issue trackers, get
them. When the page does not cancel that event, an editable region gets the
HTML with `document.execCommand('insertHTML')` and a plain field gets the text.
With nothing on the clipboard the window falls back to the engine's own paste.
The picker and the
agent tools use the page's DevTools protocol through Electron's debugger. The
developer tools are Electron's own, painted into a second page.

### Keyboard, pointer and input methods

Keys go to the page as Windows virtual-key codes (`browser_keys.rs` in
`crates/pandemonium/src/app`). A letter, digit or punctuation mark of a US
layout takes the code of the key that types it, so Control+/ or Control+[
reach the page with their key; a character of another layout, a dead key and
the media, volume and browser keys take the code of where they sit on the
keyboard; the number pad, Insert, the context menu key, the locks, the function
keys up to F24 and the modifier keys have their own codes. A key types text
only while Control, Alt and Command are up, Enter included. The helper names
each code as Electron does, punctuation by its unshifted character (`keyName`
in `src/input.js`); a key Electron has no name for, such as the context menu
key, goes through the page's DevTools protocol (`Input.dispatchKeyEvent`). On
macOS, Command and Option with the arrows, Backspace and Delete, Shift marking
what a movement passes over, are sent with the editing command macOS binds to
them (`macEditing`), since an offscreen page gets no bindings from the system.
The engine's own key events reach only the top frame, so on a page with
cross-origin frames every key goes through the protocol as well, with its DOM
key and code (`protocolKey`), which hands it to the frame that holds the
keyboard; it waits behind the pointer events sent before it
(`Router#key`), so a key pressed right after a click reaches the field the
click focused.

Pointer moves, presses and releases go through the page's DevTools protocol
(`Input.dispatchMouseEvent`), routed by `src/routing.js`: in an offscreen
window the engine sends every pointer event to the top frame, so a cross-origin
frame, which runs in a process of its own, would never see the pointer move,
show a hover state or a tooltip. The router follows each cross-origin frame as a
target of its own (`Target.setAutoAttach`), finds the innermost one under the
pointer and sends the event to that frame's target at the point inside it, in
its CSS pixels; the top frame takes points in the window's pixels without the
zoom. Finding the frame takes no round trip to the page in the common case: the
router keeps the box of each cross-origin frame's element in the viewport of
the target that holds it (`DOM.getFrameOwner` once, then `DOM.getBoxModel`),
and every target that holds such a frame runs a small script in an isolated
world (`Page.addScriptToEvaluateOnNewDocument` with a `Runtime.addBinding`
binding) that reports when its layout may have moved: a scroll, a resize, a
change to the document, an animation, a load or the pointer entering or
leaving an element. It reports the first change at once, before the change is
painted, then at most once an animation frame while changes go on, so an event
the reader sends after seeing a change finds the boxes stale. A report that
moves a frame, or `Page.frameResized`, marks that target's boxes stale and has
them measured again; an event that arrives meanwhile waits for one fresh
measure, and a box that cannot be measured, or is turned or skewed, is found
the slow way, with `DOM.getNodeForLocation`. A point outside every
cross-origin frame's box goes to the target that holds the frames with no
round trip. A point inside one is decided exactly: the router asks that
target's script, in its isolated world (`Runtime.evaluate`), what
`document.elementFromPoint` finds there, one round trip, and only a frame's
own element sends the event into the frame; anything laid over it, such as a
menu or a dialog, takes the event, however small. When the pointer comes into
the frames' boxes from elsewhere, the move also goes to the target that holds
them, sent beside the question, and the script answers once that move is
handled, so an element a `:hover` rule shows over the frame takes the pointer
too. Moves that arrive while one is being decided are coalesced to the newest,
so a burst costs at most one round trip at a time. A pressed button
keeps the pointer in the frame it went down in, and moving off a cross-origin
frame sends it a move just outside it, so it hears the pointer leave. The
agents' click, hover and drag tools use the same router.

What an input method composes in a page goes to it as `Compose`, which the
helper shows with `Input.imeSetComposition`; finished text comes as
`CommitText`, and empty `Compose` text ends a composition without typing.
`CommitText` and `PasteText` are typed with the protocol's `Input.insertText`,
which goes to the frame that holds the keyboard; Electron's own `insertText`
crashes the page when that frame is a cross-origin frame, so it serves only a
page without the protocol.
After each update the helper reports `Caret`, where the caret or the end of the
selection is in the frame that holds the keyboard (or that frame's focused
element), moved by the offset of that frame in the top frame (see
[Frames inside a page](#frames-inside-a-page)), and the window puts the
candidate window there. Composing needs the
page's protocol, so sealed pages take only finished text.

The primary, secondary and middle buttons go to the page with their presses,
drags and releases, and the page follows the pointer while any of them is held,
wherever it goes in the window. Releasing a secondary or middle button outside
the window is not seen. The wheel and trackpads send their exact distance
(`exact_dx`, `exact_dy` on `Scrolled`, with the whole pixels in `dx`, `dy` for
older helpers), with `precise` set for a turn that is not a whole number of
wheel notches, which the window counts as 48 logical pixels; the helper passes
wheel ticks and `hasPreciseScrollingDeltas` on so pages scroll smoothly.

The page's cursor maps to the closest one the window can draw: the arrow, the
hand, the text caret, the crosshair (also for `cell`), busy (also for
`progress`), the four resize handles, grab and grabbing, move (also for
`all-scroll` and panning), and not-allowed (also for `no-drop`). Cursors the
window has no shape for, such as `zoom-in`, `copy`, `alias` and `none`, show
the arrow.

When the app's main window loses the keyboard to another application, every
page is blurred as if it lost the keyboard, and the page that held it is
focused again when the window comes back.

## Opening a dev server

When a program started in a shell serves a web page, the browser opens on it.
The server that runs the shell finds the page (`crates/pm-server/src/serving.rs`,
wired into `terminals.rs`), so it works for remote projects too, and tells the
window with `terminal_serving`. It takes four agreeing signals:

1. a line of the shell's output holds a bare `http://` address of a local port,
   such as vite's `Local: http://localhost:5173/`;
2. the port was not listening when the reader last pressed Enter in that shell;
3. the port listens now (`/proc/net/tcp` on Linux, `lsof` elsewhere);
4. `GET /` on it answers with an HTML document that is not a redirect, a 404 or
   a browser debugger's discovery endpoint.

A program that no shell shows the server, such as a dev server an agent starts
with its own tools, is found by watching the ports: when one starts to listen,
the server looks for the program that holds it, and when that program works in a
session's folder and the port answers with a page, the
window is told with `session_serving`.

The window then opens the browser on the address, or sends an open browser to
it, over the same SSH forward as any other page of that machine. It happens
once for each address a command prints. A tab that is open on another
page is left there when a different server starts, since the reader or an agent
is using it, and the server's page opens in a new tab behind it (see
[Tabs](#tabs)); the tab shown goes to a server's page when it shows nothing or
an error, and every tab on a page of that same server loads it again where it
is (`served_move` in `pm-components/src/browser.rs`). A page is the same server's by the address the server reports, so `127.0.0.1` and `[::1]` count as `localhost`, and a tab on a remote machine, whether carried over SSH or reached directly, is compared by the remote address it shows, not the local one that carries it. It happens only while the embedded browser is on
and installed, and Settings, under Browser, can turn it off. A shell in a dev
container is not seen, since its ports are the container's.

## Tabs

The pane has a strip of tabs above the address bar: a chip for each tab, with
its title (or address while it has none), a close button, and a button for a
new tab. Ctrl+T and Ctrl+W open and close one while a page has the keyboard.
Each tab is its own `View` in the helper with its own history, zoom, find bar,
picking mode, permission questions and certificate page, and keeps a number for
as long as it lives; the window drives the views by that number
(`Web::pages` in `app/browser.rs`) and drops a view when its tab is gone. The
address bar, the buttons and the developer tools belong to the tab shown, and the
developer tools move to another tab when it is shown. At most 32 tabs are open;
when the strip cannot give each at least 84 pixels it scrolls, and showing a tab
scrolls the strip to it. Only the tab shown is painted into the window and
resized; the others keep their last picture.

A page that asks for a new window (`window.open`, a link with `target=_blank`)
no longer loads in place: the helper opens nothing and reports `OpenRequested`
(page, address, and whether a tool of an agent was running on the page), and the
window opens the address in a new tab on the same machine as the page that
asked. The reader's own request is shown at once; one made while a tool of an
agent ran is the agent's and opens behind the tab shown. The agent restriction on
origins still applies in the helper before the request is sent.

A real popup, `window.open` with a features string (Chromium's `new-window`
disposition), is different: sign-in and payment flows need the new window to
keep `window.opener` and to close itself when it is done. The helper lets
Electron make that window, as a hidden offscreen window confined like every
other page (`popupOptions` in `helpers/electron-browser/src/lifecycle.js`),
holds it and reports `PopupOpened` (opener, the helper's number for the window,
address, and the agent flag as for `OpenRequested`). The window opens a tab for
it whose first command is `AdoptPopup` instead of a load, and the helper makes
the held window that tab's page (`adoptPopup` in `main.js`), sending its address,
title and history as it stands, since it may have loaded already.
Electron gives such a window no offscreen view of its own: it lays out at 0 by
0 and never paints, however it is sized or shown. The adopted page is therefore
sized through the DevTools protocol, with a fixed device-metrics override of the
pane's size in device pixels, and painted from the protocol's screencast, whose
frames go into the tab's surface like any other paint (`Screencast` in
`helpers/electron-browser/src/screencast.js`). The screencast's frames come at
the override's size whatever pixel ratio it emulates, and with a fixed size the
engine lays out at that size over the zoom and leaves `scale` aside. So the
override's size is the device size over the layout's share of the scale, with
the same split of zoom and pixel ratio as any page (`frameMetrics`). When that
leaves a pixel ratio of 1 and the device size, as for a popup that follows its
pane, the frames are painted as they come. Otherwise, for a mobile viewport,
one with a pixel ratio of its own or a reader's zoom past the engine's, a frame
that differs from the last only says the page changed: the helper takes a
picture at the emulated ratio (`Page.captureScreenshot`), brings it to the
device size and paints that, so the popup is as sharp as a tab of its own.
Points reach such a popup's target in plain CSS pixels, the window's pixels
over the pane's scale times the reader's zoom (`Page#topUnits`). When the strip
is full the window answers `DropPopup` and the helper closes the window; one the
window never answers is closed after 30 seconds. A blank popup that a script
fills in afterwards is allowed only for the reader's own clicks, never while a
tool of an agent runs. When the popup calls `window.close()` the helper reports
`PageClosed` and the window closes its tab. Links with `target=_blank` and
`window.open` without features still become plain tabs through `OpenRequested`.

For a remote project a tab keeps the machine its page is on, so its address is
resolved over the same SSH forward as before, and a window that restores a
saved browser opens every tab on its own machine.

The tabs are kept in the window's saved state, for each session, in the order of
the strip: each with address, machine, title, the agent mark, the lock and the
pin, and the one shown (`SavedBrowserTab` in `crates/pandemonium/src/view.rs`).
A restored tab shows its saved title until its page reports one. Sealed and
private tabs are not kept. State saved before tabs, which has one address,
restores as one tab.

### Pinned tabs and the menu of a tab

A secondary press on a tab opens its menu (`browser_tab_menu.rs` in
`pm-components`): New Tab, New Private Tab, Reload, Pin Tab or Unpin Tab, Close
Tab and Close Other Tabs, which leaves pinned tabs open. A pinned tab moves to the
end of the pinned tabs at the start of the strip and is drawn compact, its glyph
alone, 36 pixels wide (`PINNED_WIDTH`); unpinning puts it right after them. The
strip always draws the pinned tabs and scrolls only through the others
(`Browser::strip_window`). Pins are kept across a restart, and a restored browser
puts its pinned tabs first.

### Private tabs and profiles

Every page lives in a profile of the engine, its session of a partition, which
holds its cookies, storage, cache and sign-ins. `Open` names it in `partition`
(`View::open_in` in `pm-browser`); the window picks it for each tab when its page
opens (`WorkspaceState::browser_partition` in
`pm-components/src/browser_profiles.rs`):

- A private tab, from Ctrl or Cmd+Shift+N or the menu of a tab, lives in
  `private`, a profile the engine keeps in memory only. The strip marks it with a
  detective. Nothing it visits or zooms is remembered (see
  [What the browser remembers of sites](#what-the-browser-remembers-of-sites)),
  a closed private tab is not offered to open again, and it is not kept across a
  restart. A link or a window a private page opens is a private tab too
  (`browser_tab_inherit`). When the last page in a profile kept in memory
  closes, the helper empties it: cookies, storage, cache and HTTP authentication
  (`release` in `src/partitions.js`).
- With "A browser profile for each project" on under Browser in Settings
  (`project_browser_profiles` in `GeneralSettings`), the tabs of a session live
  in `persist:project-<hash>`, kept on disk, the hash the FNV-1a of the project's
  address (`Project::path_hash`), so a sign-in in one project is not seen by
  another and the profile is the same after a restart. A browser shown outside
  any session uses the project the page shows. The setting applies to pages
  opened after it changes.
- Otherwise a page lives in the profile every page shares, as before.

The helper makes each profile the first time a page asks for it and gives it the
guards of permissions and downloads (`onSession` in `src/partitions.js`). A popup
lives in its opener's profile. A helper announces the feature `partitions`
(`PARTITIONS` in `wire.rs`); the window opens a page in a profile other than the
shared one only on a helper that announced it, and otherwise shows the tab a
notice to update the browser, so a private page never lands in the shared
profile.

### Tabs belong to a session

Every session has a browser of its own: its tabs, the tab shown, its developer
tools and the width they were given. One helper serves the whole window, with a
page for each tab of every session, so a page kept by a session you left is still
there, with its history, when you come back. Tab numbers are unique in the window,
which is how the pages of different sessions are told apart.

The browser of the session shown is `WorkspaceState::browser`, which the rest of
the window reads and changes; the browsers of the other sessions wait in
`WorkspaceState::browsers`, by the `scope` of the session that owns them, a
number that stays with a session however the sessions are renumbered
(`pm-components/src/browser_scope.rs`). Opening another session, or a page that
is no session's, swaps them, and a browser whose session is gone is dropped.

Work on the tabs of a session the reader is not looking at, such as an agent's
tool call or a dev server found for it, enters that session's browser and leaves
it again, with the page the reader is on unchanged (`browser_scope_enter` and
`browser_scope_leave`).

An agent can only reach its own session's tabs. The server gives each session its
own token for the tools endpoint when its agent starts, so every call says which
session made it (`browser_call`, see [protocol.md](protocol.md#browser-tools)).
The window runs `browser_tabs` and every other tool on that session's tabs, and
`tab` names a tab among them: a number from another session is the same as a
number that does not exist. A dev server found for a session, from a shell of
the session or by watching the ports, opens in that session's browser only.

The saved state keeps the browser of the session that was shown, and in
`session_browsers` the others by machine and the server's number for the session;
a window restores each as the server lists its sessions. State saved when one
browser belonged to the whole window restores into the session that was shown.

### Locking a tab against agents

The padlock in the address bar locks the tab shown. While it is locked the tools
that change a page are refused with a message that says to open a tab of one's
own: every tool except `browser_snapshot`, `browser_screenshot`,
`browser_console`, `browser_get_bounding_box`, `browser_wait`,
`browser_record` and `browser_network`, which only read (`changes_page` in
`pm-components/src/browser_tabs.rs`; a tool nobody listed counts as changing the
page). Closing a locked tab by `browser_tabs` is refused too. A tab an agent
opened carries a sparkle mark in the strip and cannot be locked: agents may
always work in it. The check is made by the window, before a call reaches the
helper, so it holds for agents of remote servers as well.

### Tabs for agents

`browser_tabs` is a tool handled by the window alone; the helper never sees it.
`action` is `list` (every tab: number, title, address, and marks for the tab
shown, the tools' tab, agent tabs and locked tabs), `open` (an agent's tab
behind the one shown, on `url` or blank, which then is the tools' tab), `select`
(makes `tab` the tools' tab without changing what the reader sees) and `close`
(`tab`). Every other tool takes an optional `tab`. Without it a tool works on the
tab an agent last opened, selected or named, and when there is none on the tab
shown, so an agent that never uses tabs works as before, in the tab the reader
looks at. `browser_navigate` with no tab open opens one marked as an agent's.
The helper ignores `tab` (`agent.js` deletes it from the arguments).

## WebGL

Pages get WebGL and WebGL2. The helper leaves hardware acceleration on, so a
machine with a GPU runs WebGL on it, and it passes `enable-unsafe-swiftshader`
so that a machine with no GPU, such as a headless or remote Linux box, falls
back to Chromium's software renderer. Recent Chromium no longer offers that
fallback to WebGL unless the switch is given. The helper used to call
`app.disableHardwareAcceleration()`, which left WebGL only the software path
and, without the switch, no context at all (`getContext('webgl')` returned
`null`). The page still paints through the offscreen window into the surface
files; the window never reads GPU textures.

## When a page counts as loaded

`browser_navigate`, `browser_back`, `browser_forward` and `browser_reload` end
when the page has loaded and then settled: no request of the page is on its way
and the network has been still for half a second, or five seconds have passed.
A move inside one document, which single-page apps make, counts as arriving
without a load. A page that fails to load ends the call with the engine's
reason. The answer says the address, the title and the HTTP status of the
document. `browser_screenshot` also waits, for up to two and a half seconds,
for requests under way to finish. Sockets and event streams stay open for as
long as the page does, so they are not waited for.

## What agents can do with a page

Besides navigating, reading and clicking, the tools can hover (`browser_hover`),
drag (`browser_drag`, by element or point, through the same router as clicks
and hovers, so a drag inside a cross-origin frame reaches it; HTML drag and drop
through `Input.setInterceptDrags` on the page and every frame target, dropped on
the frame under the end point, other drags as plain mouse events), fill several
fields at once (`browser_fill_form`, which runs the same code as `browser_type`,
`browser_select_option` and a click on a box, with the same refusal messages and
a stop at the first failure), outline an element for the person watching
(`browser_highlight`, a fixed overlay that removes itself), measure one
(`browser_get_bounding_box`), take a picture of the whole page or one element
(`browser_screenshot` with `full_page`, `ref` or `selector`; it uses
`captureBeyondViewport`, and a full-page picture is cut at 16000 pixels) and set
the viewport size (`browser_resize`).

`browser_resize` sizes the window to the viewport and emulates its screen with
`Emulation.setDeviceMetricsOverride` (see Display scale), so the size stays
through navigation. It does not fight the pane: a `Resize` request of the pane
keeps the window at the viewport's size, so the page is shown at the emulated
size at the top left of the pane and the rest of the pane is empty. `reset`
gives the size back to the pane.

### The console and the network

The helper's recorder (`helpers/electron-browser/src/recorder.js`) keeps the
latest 200 console entries and requests of each page; `netview.js` filters
them and writes them for agents.

Each console entry has a number that counts up for the page, the time it was
written, its level, its source (`address:line:column`, from the protocol's call
stack or location) and, for uncaught exceptions, `console.error`,
`console.assert` and `console.trace`, its call stack, at most 12 frames.
`browser_console` writes an entry as `#12 10:04:05.120 [error] Uncaught
TypeError: … (http://…/app.js:10:5)` with the stack indented below it, and ends
every answer with `[cursor: N]`: given back as `since`, it returns only the
entries written after that answer, so an agent can poll without reading old
entries again. Pictures of the console added to the prompt use the same
entries.

`browser_network` lists requests as `#number METHOD url -> status (type)`.
`type` keeps resource types as the engine names them (`xhr`, `fetch`,
`document`, `script` and so on, several separated by commas), alongside
`filter`. When a request of a text type finishes, the recorder reads its body
at once with `Network.getResponseBody`, while the engine is sure to have it, and
keeps it cut to 8000 characters, so a body is still there after the page
navigates. The kept bodies are bounded: past about a million characters the
oldest are dropped, and their request says so. What the request sent (its
`postData`, read with `Network.getRequestPostData` when the event leaves it out)
is kept too, cut to 4000 characters, and shown with `request`.

`wait_for` makes `browser_network` wait until a request whose address matches
finishes, then gives it in full like `request`. The pattern is text the address
contains, or, with `*` in it, a pattern over the whole address where `*` is any
run of characters. A matching request that finished since the previous tool
call started counts too, so a click followed by `wait_for` does not miss a
response that came back during the click. `type` narrows it and `timeout_ms`
(10 seconds by default, at most 60) bounds the wait.

### Recording what agents do

`browser_record` with `action` `start` begins a recording
(`helpers/electron-browser/src/recording.js`); `stop` ends it and answers with
its summary, and `status` says whether one is under way. Recordings are kept
per agent session: the window names the session that made each call in the
`session` and `session_name` fields of `Tool` (`session-<scope>` and the
session's title, `browser_caller` in `pandemonium/src/app/browser.rs`), and
`browser_record` starts, stops or describes only the recording of the session
that calls it. While a session's recording runs, every tool call of that
session is kept as a step with the tool, its arguments (cut to 2000
characters), whether it worked, the start of its answer, the page's address and
a PNG of the page taken after it (skipped while a dialog stops the page, and
after five seconds); other sessions' calls are not. `browser_tabs`, which the
window answers itself, is handed to the helper afterwards as `RecordStep`, with
what it answered and the page the session's tools now work on to picture; with
no such page the step is kept without a picture. `browser_record` itself is not
a step. The steps go in the folder the agent named, which must be absolute and
not one another session records into, or else in a folder named by the time,
the session's title and its key under `recordings` in the helper's data folder:
`NNN-tool.png` for each step, `steps.json`, and `summary.md`, a numbered list
that links the pictures, rewritten after every step. A recording stops itself
after 500 steps.

Secrets are never kept (`src/secrets.js`). Before a recorded `browser_type` or
`browser_fill_form` runs, the helper asks the page whether each field it types
into is a password field or has `autocomplete` `current-password`,
`new-password` or `one-time-code`; the text typed into such a field is kept as
`[redacted]`, and the field stays marked as secret in the tools' scripting
world, even if the page later turns it into a plain field. A field that cannot
be checked counts as secret. Any argument whose name looks like a password,
token, secret, API key, credential or cookie is redacted whatever the tool.
Before each step's screenshot every secret field is blurred and given
`-webkit-text-security: disc`, and put back as it was after; a screenshot taken
when the fields could not be blurred is not kept.

Each step also travels to the window in the `step` field of the tool's
`ToolDone` result (`RecordedStep` in `pm-browser/src/wire.rs`), with the
session's key and name. The window reads the step's picture, leaving the file
where it is, and adds the step to the `steps` of the browser that holds the tab
it ran on (`pm-components/src/browser_steps.rs`), so each session's steps show
in its own browser. The first step of a session's new recording drops that
session's older steps and opens the panel of steps beside the page; a film
strip button in the toolbar opens and closes it while there are steps. While
the panel holds steps of more than one session, a row above it lists **All
sessions** and each session by name, to list one session's steps alone. The
panel shows the screenshot of one step with its tool, session, arguments,
answer and address, buttons that step back and forth through the steps listed
to replay the work, and below them every step listed with a thumbnail;
pressing one shows it, and **Latest** goes back to following each new step as
it arrives. The `browser-steps` snapshot shows the panel.

### Scripts, contexts and recipes

`browser_run` runs a whole flow in one call and answers only with what the
script returns (`src/runner.js`). `script` is the body of an async function
run in a Node `vm` context of the helper, which holds nothing but the API: the
tab of the call as `page`, `browser.newContext`, `check`, `log`, `sleep` and
`parallel`. Each page method, such as `goto`, `click`, `fill`, `type`,
`select`, `waitFor` or `screenshot`, runs the browser tool of the same name on
that page, through the same origin restriction, and throws the tool's text when
it fails. The answer is JSON: `ok`, `value` (what the script returned),
`error`, `checks` with `passed` and `failed`, `logs`, `steps`, `ms`, `recipe`
and `recording`. `ok` is false when the script threw, ran past `timeout_ms`
(60 seconds by default, at most 600), or a check failed. `base_url` makes
relative `goto` addresses whole; the window resolves it as it resolves
`browser_navigate`'s `url`, so it is how a script reaches a remote project's
server.

Requests are data in scripts: `waitForResponse(pattern)` waits as
`browser_network`'s `wait_for` does, a request that finished since the step
before started counting, and gives `{ url, method, type, status, failure,
finished, body }`; `requests(pattern)` lists what the recorder holds, and
`count(pattern)` gives `{ total, statuses, failed }`, `statuses` mapping each
status as text to how many had it.

`browser.newContext({ device, viewport, locale, timezone, userAgent,
headless })` makes a context: a profile kept in memory named `context-<pid>-<n>`
that no tab and no other context shares, so its cookies and storage start
empty, emptied when the context closes or the run ends (`partitions.release`).
`device` takes the presets of `browser_emulate`; `locale` sets
`Emulation.setLocaleOverride` and the `Accept-Language` header, `timezone`
`Emulation.setTimezoneOverride`. A context is headless unless `headless` is
false: its pages are pages of the helper that no tab shows, numbered below
zero, which report nothing to the window. A context shown in the pane asks the
window for each page as a page asks for a new window, `OpenRequested` with the
context's `partition`; the window opens an agent's tab behind the one shown in
that profile (`BrowserTab::context`), leaves the tools' tab as it was and never
saves the tab, and the helper hands the tab's page to the script when its
`Open` arrives. Closing such a page reports `PageClosed`, which closes the tab.
At most `limit` pages of contexts are open at once (4 by default, at most 8);
`newPage` waits for a free one, and `parallel(items, fn, limit)` runs `fn` on
the items that many at a time, giving each `{ ok, value }` or `{ ok: false,
error }`.

Clicks and typing can look like a person's: `browser_click` with `human`, and
`click` in scripts with `{ human: true }` or the run's `human`, scrolls the
element into view smoothly, waits for the scroll to stop, glides the pointer
there along an eased path from where it last was, and lingers before pressing
and letting go. `browser_type` with `delay_ms`, and `type` in scripts (80 ms by
default), sends a key press and release for each character with about that
delay between them, varied by a third.

`save` keeps the script, with `base_url`, `human`, `limit` and `timeout_ms`, as
a recipe in `recipes/<name>.json` of the helper's profile folder; `recipe` runs
one by name, `list` lists them, and `record` records each step of the run as
`browser_record` does, named in `recording`. The list button in the toolbar
opens the panel of recipes (`pm-components/src/browser_recipes.rs`): it lists
them through `browser_run` with `list` on the tab shown, and **Run** runs one
recorded on that tab as the session `recipes`, with no agent; the answer goes to
the panel, not to a server, and the panel shows whether it passed, a row for
each check with its mark and detail, and the folder of its recording. The
`browser-recipes` snapshot shows the panel.

## File pickers

A click on `<input type=file>` makes Electron open a native dialog parented to
the page's window, which here is hidden and offscreen, so the dialog never
shows and the input stays empty. `helpers/electron-browser/src/filechooser.js`
turns on `Page.setInterceptFileChooserDialog`, opens `dialog.showOpenDialog`
itself with no parent when `Page.fileChooserOpened` arrives, and hands the
chosen files to the input with `DOM.setFileInputFiles`. On macOS the helper has
no Dock icon and so counts as a background app, whose dialog opens behind other
windows or not at all, so it calls `app.focus({ steal: true })` first. The files are the
reader's own, on the machine the window runs on. Agents still cannot use either
command (see `cdp-policy.js`).

Before the dialog opens the helper reads the input's `accept` and
`webkitdirectory` through `DOM.resolveNode`. An input with `webkitdirectory`
gets a folder dialog. `accept` becomes one filter of the extensions it names
(`.pdf`, or common types such as `image/*` and `application/json`, `filtersOf`)
followed by "All files", since `accept` is only a hint. A picker a page opens
while a dialog is already open waits its turn in a queue instead of being
dropped; the same input asking twice is asked once.

## Dropping files on a page

Files dragged from a file manager and let go over the page go to the page, so
upload drop zones work. The pane's window of the page is a rustle drag and drop
target for `DragData::FilesList` (`page_file_drop` in
`crates/pm-components/src/browser_pointing.rs`); on the drop the window sends
`DropFiles` with the paths and the point of the page as a `Chrome` action, and
`src/drop.js` in the helper dispatches `dragEnter`, `dragOver` and `drop` with
the files through the protocol's `Input.dispatchDragEvent`. The desktop hands
the paths over only on the drop, so the page sees no drag while the files are
held over it. A page that does not take the files would open the first one,
which the guard that keeps the page on web addresses stops. Sealed pages, reader
view and the developer tools take no files.

## Drop-downs and date pickers

Chromium draws the popup of a `<select>` and of a date, time, date-and-time or
month input as a separate widget, and an offscreen Electron page never paints
it (checked on Electron 45: no frame arrives after the click). So
`helpers/electron-browser/src/popups-preload.js`, a preload of every page that
runs in its isolated world, catches the press and draws a popup of its own in a
closed shadow root, which is painted with the page. Choosing sets the value and
fires `input` and `change`. A press on the picker button at the right end of a
date input opens the picker; the rest of the input still edits by typing. Lists
with `multiple` or a `size`, and `week` and `color` inputs, are not covered.
The preload runs in every frame of the page (`nodeIntegrationInSubFrames`,
which with `sandbox: true` gives the frames the preload and no node), so a
list or picker inside a frame opens too; its popup is drawn in that frame and
is cut off at the frame's edges.

### Frames inside a page

A frame reports a point, such as the pointer under a tooltip or the caret, in
its own viewport. `helpers/electron-browser/src/frames.js` (`offsetOf`) finds
where that viewport sits in the top frame, one parent at a time: the child
finds its place among its parent's frames (`window.parent[i] === window`), and
the parent finds the `<iframe>` or `<frame>` whose `contentWindow` is that
child and gives where its content box starts. Both checks are allowed across
origins, so the same steps serve same-origin and cross-origin frames.

## Dialogs

`alert`, `confirm`, `prompt` and `beforeunload` are handled in
`helpers/electron-browser/src/dialogs.js`. What Electron does with them in our
offscreen pages, read from its source (`electron_api_web_contents.cc` and
`lib/browser/api/web-contents.ts`):

- `alert` and `confirm` stop the page's renderer and ask `dialog.showMessageBox`
  for a native box. With an offscreen page there is no parent window, so the box
  would be free-standing and nothing in the pane could answer it. While the
  renderer is stopped, DevTools commands such as `Runtime.evaluate` never answer,
  so an agent's call would hang with it.
- `prompt` is cancelled at once, without a box: the page gets `null`. The
  preload (`popups-preload.js`) therefore puts its own `window.prompt` in the
  page's world (`contextBridge.executeInMainWorld`), which asks the helper with
  a synchronous IPC message (`pandemonium:prompt`). The page waits on it as on a
  real prompt; `Dialogs.askPrompt` keeps it open like the other dialogs and
  answers it with the text, or `null` when cancelled. A frame inside the page
  gets the same prompt; the reply goes to the frame that asked, and the sheet
  shows that frame's address. In a frame Electron puts its own `prompt` back
  after the preload ran, so the preload puts the helper's in again on the
  first change to the document, before the page's scripts run.
- `beforeunload` emits `will-prevent-unload`; if nobody calls `preventDefault`,
  the navigation is cancelled silently (`did-fail-load` with `ERR_ABORTED`).
- The DevTools protocol reports `alert` and `confirm` with
  `Page.javascriptDialogOpening` and answers them with
  `Page.handleJavaScriptDialog`.

So the helper replaces `dialog.showMessageBox` for page dialogs with a promise
that the helper settles itself (nothing native opens), records an open dialog
from the protocol, and has `browser_handle_dialog` answer it. While a dialog is
open every tool that talks to the page (everything but `browser_console`,
`browser_network` and `browser_record`) returns at once with the dialog described instead of
hanging; a tool that makes the page open one returns as soon as it does. Every
answer ends with a "Dialogs" list of what is open and what closed since the agent
was last told. A dialog nobody answers is dismissed after 60 seconds. While a
tool is running, a `beforeunload` prompt is overridden so the tool can leave the
page; at other times Electron's cancel stands, and the agent is told.

When no tool of an agent runs on the page, the reader's own click made the
dialog open, so the helper reports it as `Chrome { report: Dialog }` with a
number, its kind, message, starting text and the page's host. The pane draws a
sheet over the dimmed page ("example.com says", the message, a line for a
prompt's answer, OK and, for `confirm` and `prompt`, Cancel;
`crates/pm-components/src/browser_chrome.rs`). Enter answers OK and Escape
Cancel. The answer goes back as `Chrome { action: AnswerDialog }`, which the
helper takes only while that dialog is still the open one. When the dialog
closes another way, such as an agent answering it or the timer, the helper
reports `DialogClosed` and the sheet goes away. A page that opens more than
three dialogs in 30 seconds has the rest dismissed unseen.

Known limits: the patch of `dialog.showMessageBox` relies on Electron calling
it through the module object, as it does at the time of writing; if it did not
take, the native box would open and the protocol could still answer it. A page
that keeps a reference to `window.prompt` taken before the preload ran keeps
Electron's, which returns `null`.

## Keeping the reader safe

Pages are not trusted. The helper (`src/safety.js`, `src/origins.js`) refuses by
default and tells the window when it needs the reader's say; the window shows
it in the pane (`crates/pm-components/src/browser_safety.rs`) and sends the
answer back. What the reader answers is remembered by the helper only until it
ends, and never written to disk.

- **Permissions.** Camera, microphone, location, notifications, clipboard
  reads, MIDI, pointer lock, idle detection and window management are asked
  for, per origin and permission, in a bar above the page with Allow and Block
  (`PermissionAsked`, answered with `PermissionAnswer`). Everything else, such
  as opening other programs, USB, serial, the screen and requests from frames
  other than the top one, is refused without asking. A question nobody answers
  within two minutes, or whose page moved on, ends as a no and is not
  remembered. A remembered Block always holds; a remembered Allow is not used
  while a tool of an agent runs on the page, so a script an agent sets off asks
  again. Answers are remembered per profile and origin: what the reader allowed
  in one profile, such as the shared one, a project's or the private one, is
  asked again in another, and the answers given in a profile kept in memory
  only, such as the private one, are forgotten when its last page closes.
  Putting text on the clipboard in answer to a click is still allowed.
- **Certificates.** A site whose certificate is not trusted does not load. The
  helper keeps the engine's refusal (`certificate-error` is answered no) and
  reports `CertificateError` with the address, the engine's error name, who the
  certificate is for, who issued it, its SHA-256 fingerprint and when it
  expires. The pane shows "Not secure: certificate not trusted" in place of the
  page. Go back sends `CertificateChoice { proceed: false }`. "Proceed anyway
  (unsafe)" needs two presses and sends `proceed: true`; the helper then trusts
  that origin with that certificate until it ends and loads the address again.
  A certificate error in anything but the page's own document is refused and
  never offered.
- **Downloads.** A download is saved without a dialog into the reader's
  Downloads folder. The helper takes only the last part of the proposed name,
  replaces characters file systems treat specially, drops leading dots, avoids
  names Windows reserves, and creates the file exclusively, adding ` (1)`, ` (2)`
  before the extension when the name is taken, so nothing is overwritten.
  Pages and their developer tools may download; sealed pages may not. Each
  download gets a number and is reported as `Chrome { report: Download }` with
  the bytes received and in all, at most four times a second, and once more
  when it finished, failed or was cancelled. The toolbar then has a downloads
  button, accented while one runs, which shows the list over the top right of
  the page: each with its name and progress, a button to cancel a running one
  (`CancelDownload`; the partial file is removed), and for a finished one buttons
  to show it in its folder (`ShowDownload`) or open it (`OpenDownload`). Nothing
  is opened unless the reader asks. A new download opens the list.
- **Sharing the screen.** `getDisplayMedia` from a page's top frame, while no
  tool of an agent runs on it, lists the screens and windows
  (`desktopCapturer`) and reports `Chrome { report: ScreenShare }`. A bar over
  the top of the page offers a button for each and "Don't share"; the answer is
  `ChooseScreen` with the source or none. A question not answered within two
  minutes, or whose page closed, is refused and reported as
  `ScreenShareCancelled`.
- **Origins for agents.** Settings, under Browser, has "Restrict agent browsing"
  (off by default) and a list of origins. While on, the tools of agents may take
  the page only to `localhost`, `127.0.0.1`, `[::1]` (any port) and the listed
  origins. An entry with a scheme, such as `https://docs.rs`, is that origin; one
  without, such as `staging.example.com:8443`, is that host and port over
  `http` and `https`, and without a port the usual one. The window sends the
  setting with `AgentOrigins` when the helper starts, before any page, and
  whenever it changes. The helper checks it in `mayOpen` (`agent.js`) for
  `browser_navigate`, `browser_back`, `browser_forward` and `browser_reload`, and
  `Page.guard` checks it on `will-navigate`, `will-redirect` and new windows
  while a tool of an agent runs on the page, so a redirect or a link a click
  follows cannot leave the list either. The reader's own moves are not limited.
  A refusal says which origin was refused and where to change it.

The messages for all of this were added to `wire.rs` and `types.js` after the
existing ones; nothing before them changed.

## Ads and trackers

Settings, under Browser, has "Block ads and trackers", on by default
(`block_trackers` in `GeneralSettings`). The window sends it, with the setting
for passwords, as `Privacy { block_trackers, save_passwords }` when the helper
starts and whenever either changes; a helper that announces
`content-blocking` in `Features` carries it out.

The helper (`src/blocker.js`, with the decisions that need no engine in
`src/blocker-policy.js`) builds its engine with `@ghostery/adblocker-electron`
from EasyList and EasyPrivacy, fetched from `easylist.to` with the engine's own
network stack. It keeps the built engine in `content-blocking/engine.bin` under
its profile, with when the lists were fetched in `lists.json`, so a start reads
the file instead of fetching. Once an hour it looks whether the lists are a day
old and fetches them again; a failed fetch keeps the engine it had and is tried
again ten minutes later at the earliest. Until the first engine is built,
nothing is blocked.

In every profile pages of the web live in, `webRequest` hands each request to
the engine: a request the lists name is cancelled, or answered with the
harmless stand-in the lists give for it, and a document gets the content
security policy the lists ask for. The engine's preload asks for the rules that
hide elements of each frame, which the helper inserts as user styles. A page's
own document is never blocked. Nothing is blocked on a page whose top frame is
on this machine (`localhost`, names under `.localhost`, `127.0.0.0/8`, `[::1]`
and `0.0.0.0`), so a dev server is never changed, nor on a site the reader let
through. Sealed pages live in a session of their own that never sees the
blocker.

The helper counts the requests it stopped on each page since its document
loaded and reports them, at most four times a second, as
`Chrome { report: Blocking { on, blocked } }`, also after each navigation. The
toolbar then shows a shield (`browser_privacy.rs`): accented with the count
while it blocks, and crossed out while the site is let through; its tooltip
says what pressing it does. Pressing it sends
`Chrome { action: SiteBlocking { on } }`, which lets the page's host through,
or blocks on it again, keeps the choice in `content-blocking/allowed-sites.json`
and loads the page again.

A stopped request fails in the page as `net::ERR_BLOCKED_BY_CLIENT`. It never
reaches the network, so the developer tools show it as blocked and the network
tool of agents (`browser_network`) does not list it; every request that does go
out is recorded and inspected as before. Agents see the same page as the
reader: the blocker works while their tools run too.

## Saved passwords

Settings, under Browser, has "Offer to save passwords", on by default
(`save_passwords` in `GeneralSettings`), carried out by a helper that announces
`passwords`.

A preload of its own, `src/login-preload.js`, runs in the isolated world of the
top frame of every page of the web; sealed pages never run it. It finds the
login form around a field: the first password field of its form, or of the
page when it is in none, and the field with `autocomplete` `username` or
`email`, else the last text, email or telephone field before the password. It
tells the helper (`src/passwords.js`) when the reader sends one: by submitting
the form, pressing Enter in a field of it, or pressing a button while its
password holds something. The page's next navigation within a minute, a new
document or a change of its address in place, then offers to save the login as
`Chrome { report: OfferPassword { id, origin, username, update } }`, unless the
origin was refused or the login is saved as it is. The pane shows a bar over
the top of the page: "Save the password of ada for example.com?" with Save,
Never for this site and Not now, or Update when the user name is saved with
another password. The answer is `Chrome { action: AnswerPassword { id, save,
never } }`. An offer nobody answers closes after two minutes
(`PasswordOfferClosed`).

Logins are kept by origin in `passwords.json` under the helper's profile,
readable by its owner alone, by `src/vault.js`. Each login, user name and
password together, is sealed with Electron's `safeStorage`, whose key the
system's keychain holds: the Keychain on macOS, DPAPI on Windows, and on Linux
the Secret Service or KWallet. Only the origin is written in the clear.

Where the keychain protects nothing, when `safeStorage` is unavailable or, on
Linux, its backend is `basic_text` (a desktop without a keyring, where it would
fall back to a key every install shares), the logins are kept instead in
`passwords-primary.json`, sealed by a primary password the reader chooses
(`src/primary.js`). Its key is derived with scrypt (N 2^15, r 8, p 1, a random
16-byte salt) and seals each login with AES-256-GCM under a fresh nonce.
`passwords-primary.key.json` holds the salt, the cost and a verifier, a known
text sealed with the key, never the password or the key. The helper logs which
of the two it uses when it starts.

The key lives in the helper's memory only, so the vault is locked whenever the
helper starts. When a login is to be saved, or a login field takes the keyboard
on an origin with saved logins, while it is locked, the helper asks the pane
with `Chrome { report: PrimaryPassword { create, origin, problem } }`: `create`
the first time, when none is set. The pane shows a sheet over the page
(`browser_privacy.rs`) with a hidden line, typed twice when choosing one, that
takes the keyboard; the answer is `Chrome { action: PrimaryPassword { password }
}`, `null` when the reader turns it down with Cancel or Escape. A refused
answer, shorter than eight characters or not the password set, is asked again
with `problem` saying why; a right one unlocks the vault until the helper ends,
reports `PrimaryPasswordClosed` and carries out the save or shows the list of
logins. A page whose reader turned the question down is not asked again on a
login field before it navigates. Turning the setting off closes the question.

When a field of a login form takes the keyboard on an origin with saved logins,
the preload sends where the field is, and the helper reports
`Chrome { report: LoginField { x, y, width, height, usernames } }`, the box in
logical pixels of the page. The pane lists the user names just under the field,
with the passwords hidden; pressing one sends
`Chrome { action: FillLogin { username } }`, and the helper sends that login to
the preload, which fills the form when the page is still on that origin and
fires `input` and `change` as typing would. The field losing the keyboard to
another part of the page, the page scrolling or navigating, or the fill takes
the list away (`LoginFieldLeft`); the pane taking the keyboard does not.

Passwords never reach the window, which sees origins and user names only, and
none of this is open to agents: no tool reads the vault, a login sent or a
field focused while a tool of an agent runs on the page is ignored, a fill is
refused then, and sealed pages have no preload. A password the reader filled in
is part of the page like anything typed, which the tools that read a page can
see, as they can a password the reader types.

## Sealed pages

An agent can hand the reader HTML of its own: a prototype it shows in the
conversation (the `Show` event with kind `Prototype`) and the mockup of an
answer to a question (`mockup` of an alternative of `Inquire`; see
[protocol.md](protocol.md#event-types)). That HTML is content an agent wrote,
possibly from what it read on the web, so it is never trusted. The helper shows
it in a *sealed page* (`helpers/electron-browser/src/sealed.js`, with the
decisions that need no engine in `src/sealed-policy.js`), in two ways:

- **`RenderHtml { ticket, html, width, height, path, scale }`** renders the
  document offscreen at a viewport and display `scale` (1 when missing; the
  window sends its own, so the PNG is `scale` times the viewport and its
  density is read back from its width) and writes a PNG to `path`, whose cache
  name includes the scale when it is not 1; the answer is
  `HtmlRendered { ticket, path, error }`. The window uses it for the picture in
  a prototype's card (1024 by 640) and in the tile of a mockup (640 by 400).
- **`OpenHtml { page, html, title, width, height, scale }`** opens the document
  as an interactive page, which the window shows as a tab of the session's
  browser when the reader presses Open on a prototype's card or the enlarge
  button of a mockup's tile.

What a sealed page is held to:

- **A session of its own, in memory.** Every sealed page lives in
  `session.fromPartition('sealed-documents')`, with no `persist:` and the HTTP
  cache off: it shares no cookies, storage, cache or logins with the pages the
  reader browses, and nothing of it is written to disk. The document is loaded
  from a `data:` address, whose origin is opaque, so it has no cookies and no
  storage of its own either.
- **No network.** `webRequest.onBeforeRequest` of that session cancels every
  request whose address is not `data:`, `blob:` or `about:` (`allowed` in
  `sealed-policy.js`): fetches, images, scripts, styles, fonts, frames, web
  sockets and beacons. The session is also given a proxy that does not exist,
  for loopback addresses too, WebRTC is held to proxied connections, and the
  document gets a content security policy as its first element
  (`default-src 'none'`, inline scripts and styles only, `form-action 'none'`,
  `base-uri 'none'`, `webrtc 'block'`), put after the doctype so the page keeps
  its rendering mode. Each of these alone would stop a request; they are
  layered.
- **Scripts run, and reach nothing.** The window is `sandbox: true`,
  `contextIsolation: true`, without node integration in the page, its frames or
  its workers, and without the webview tag. An interactive document's preload
  also runs in its frames (`nodeIntegrationInSubFrames`), sandboxed like the
  page. A rendered document has no preload.
  An interactive one has only the preload that draws drop-downs and date
  pickers (see [Drop-downs and date pickers](#drop-downs-and-date-pickers)),
  which runs in the isolated world and exposes nothing to the page.
- **It stays on its document.** New windows are denied and not reported to the
  window, the top frame's navigations and redirects are cancelled, a frame
  inside it may go only to `data:`, `blob:` and `about:` addresses, a `Load`
  request is ignored, and `ClearData` does nothing.
- **Nothing is asked or saved.** Permission requests and checks are refused
  without a question, device permissions are refused, and downloads are
  cancelled.
- **Agents cannot drive or read it.** A sealed page has no DevTools protocol
  attached, so the helper refuses every tool on it, as it does for the
  developer tools. The window leaves a sealed tab out of what `browser_tabs`
  lists and resolves, so to an agent its number is a tab that does not exist,
  and the tab is locked. An agent therefore cannot use a prototype to carry
  data to a page it can read, or the other way.
- **Renders are bounded.** A document over 256 KiB (the most the harness lets
  an agent write) is refused, by the window before it asks and by the helper
  again. A render ends after ten seconds, two run at once and at most 64 wait.
  The window names the file after the SHA-256 of the viewport and the document
  (`crates/pm-browser/src/render.rs`), under `sealed-renders` in the user's
  cache folder, so a document is rendered once: a file that is there is read
  without asking the helper, even when the helper is not installed, and files
  older than thirty days are removed. In the window the picture is one of the
  figures, kept by a key of the document and the viewport, so a redraw never
  renders.
- **Nothing waits forever.** The window gives a render fifteen seconds, longer
  than the helper's own ten so that the helper's answer comes first when there
  is one, and then shows "The browser did not render the preview within 15
  seconds." with a Retry button, which asks again for every preview that failed.
  A sealed page that has not painted fifteen seconds after `OpenHtml` is closed,
  and its tab says so; reloading the tab opens the document again.

The helper runs on the machine of the window, as for every page: the document
reaches the window in the session's events, so prototypes and mockups of a
remote project render the same way, and nothing is asked of the server.

Without the helper (the embedded browser off, not installed, or not built for
the system) a prototype's card and a mockup's tile say "Install the browser to
preview" in place of the picture; the title, the labels and the descriptions
are all there, and every answer can be picked. Where the system has a helper,
the placeholder has a button (`offer` in `pm-components/src/exhibit.rs`
decides which): "Install browser" while the helper is missing, which turns
the setting on and so starts the same download as the switch in Settings,
under Browser, and "Turn on browser" while it is installed and the setting is
off. While the download runs the placeholder says what it is doing, and when
it failed it says why, with the button to download again. In a tile narrower
than 264 pixels the button stands alone, and under 140 it is only an icon.
The previews are asked for again once the browser is on and installed.

When the helper runs but did not announce `sealed-pages`, as a helper older
than the window does, the placeholders say "Update the browser to preview" with
an "Update browser" button, and the Open button of a card and the enlarge button
of a tile are disabled with a tooltip that says why. The button closes the
browser's pages, downloads the helper of the window's version again through the
same download as the switch, and renders the previews again with the new
helper. When the helper is still too old after that, as when the release of the
window's version predates sealed pages, or when `PANDEMONIUM_BROWSER_HELPER`
names an old helper (which a download never replaces), the placeholder says
what to do instead: run `make install` or `make run` from the checkout, or
point the variable at a current helper.
Diagrams need no helper: the window draws Mermaid itself.

When the reader sends a sealed tab to an address, the window closes the sealed
page and opens an ordinary one in its place, so a page of the web never runs in
the sealed session. Sealed tabs are not kept across a restart, since their
documents are not.

Known limits: `alert` and `confirm` in a sealed page open as native boxes, as
for a click of the reader's in any page; the tools that add a screenshot or the
console to the prompt, and picking an element, do nothing on a sealed page,
since they use the DevTools protocol.

The helper's policy has tests that need no engine: `npm test` in
`helpers/electron-browser` runs `node --test test/`.

## Back and forward buttons

The mouse's back and forward buttons go through the history of the page while
the pointer is over it, or it holds the keyboard and is on screen. Anywhere else
they move through the pages of the window, as before.

After each navigation, in-page navigation and stop of a load the helper reports
`History` with `navigationHistory.canGoBack()` and `canGoForward()`, and the
toolbar greys out the back and forward buttons that have nowhere to go. A helper
older than this report never sends it, and then both stay pressable
(`PageLife::can_go_back` in `crates/pm-components/src/browser_life.rs`).

## Loading, failed loads and crashes

`helpers/electron-browser/src/lifecycle.js` watches each page that is not sealed
and reports:

- `Loading { loading, progress }` on `did-start-loading`, the main frame's
  `did-start-navigation`, `did-navigate`, `dom-ready` and `did-stop-loading`.
  Electron gives no measure of how far a load got, so `progress` is a guess from
  these steps (0.1, 0.25, 0.5, 0.8, then done). While a page loads the toolbar's
  reload button becomes a stop button, which sends `Stop` (`webContents.stop()`),
  and a thin line over the top of the page grows with the progress.
- `LoadFailed { url, code, name }` from `did-fail-load` for the main frame only,
  leaving out `-3` (`ERR_ABORTED`, a load stopped or replaced) and certificate
  errors, which have their own page (see [Keeping the reader safe](#keeping-the-reader-safe)).
  The pane then shows its own error view in place of the page: what went wrong in
  words for names that do not resolve, refused connections, timeouts and being
  offline (`failure_words`), the engine's name and number, and a Try again button
  that loads the failed address again. The address bar shows the failed address.
  The view is drawn by the pane, not loaded into the page, so the page's history
  and what agents read of it are the engine's own.
- `Crashed { reason }` from `render-process-gone`, except a clean exit. The pane
  shows a view that says the page stopped, with a Reload button; loading again
  starts a new content process.
- `Responsive { responsive }` from `unresponsive` and `responsive`. A bar over the
  page says it is not responding and offers Reload.

The next load to start clears a failure, a crash and the bar. `pm_browser::Lifecycle`
holds what was reported for each page; the window hands it to the tab as
`PageLife` when it changes.

## When the helper stops

The connection to the helper closes when its process ends, which sets the
engine's failure. Each turn `browser_revive` in `app/browser.rs` looks for that:
it drops the engine and every page, answers the tool calls that were running
with what happened, and has every tab that had a page open again, at its last
address or on its sealed document, with a note over it that the browser was
started again; the next load starts a new helper. A helper that stops three
times within a minute is not started again, and every tab says so until the
browser is turned off and on in Settings.

## The address bar

What is typed in the address bar becomes an address in
`crates/pm-components/src/browser_address.rs`. Text with a scheme (`https://`,
`about:`, `data:`) is loaded as it is. A host gets a scheme: `http://` for
`localhost`, loopback addresses, `*.localhost` and `:3000`, which means
`localhost:3000`, and `https://` for anything with a dot or written as
`name:port`. Machines of the local network get `http://`: IPv4 addresses,
`name:port` without a dot (`devbox:8080`), and names ending in `.local`,
`.lan`, `.internal`, `.home.arpa` or `.test`. A single word is a host, over
`http://`, when it goes on with a slash (`devbox/`) or is a host without a dot
the reader visited before. Everything else is searched for: text with spaces,
and other single words, such as `react`. The engine is the setting "Search
engine" under Browser (`search_engine` in `GeneralSettings`: DuckDuckGo, Google,
Bing, Brave or Startpage). Nothing typed loads `about:blank`.

While the reader types in the bar, a list under it suggests up to six addresses
visited before whose address or title holds the text, those starting with it
first, then the most visited and most recent; pressing one goes there.

### What the browser remembers of sites

The server on the reader's own machine keeps the zoom of each origin the reader
zoomed and the last 500 addresses visited with their titles, in
`browser-sites` in its data folder (`crates/pm-server/src/browser_sites.rs`,
with the rules for recording and suggesting in
`crates/pm-protocol/src/sites.rs`). It belongs to the reader rather than a
project, so the window always uses the local server for it, never a project's
remote one, through the `browser_sites`, `browser_visited`, `browser_titled`
and `set_site_zoom` requests described in [protocol.md](protocol.md).

The window asks for everything remembered the first time the browser needs it
and keeps a copy in `Work`, so the address bar suggests from memory as the
reader types. It records each visit, title and zoom in its copy and sends it
to the server, which writes its file at once and tells other clients with
`browser_sites_changed`; their copies are replaced by what it sends. When a
page goes to another origin, the window puts back the zoom remembered for it
(`SetZoom`); a zoom the reader then changes is remembered for that origin, and
100% forgets it. A `browser-sites.json` an earlier version of the window kept
beside the helper's profile is taken over by the server the first time it
reads the store, and removed.

Pages of private tabs take no part: nothing they visit, title or zoom is
recorded, and no remembered zoom is put back on them
(`browser_chrome_sites` in `crates/pandemonium/src/app/browser/chrome.rs`).

## Finding, zooming and loading again

These work while the page holds the keyboard and are also rows of the menu over
the page (`browser_menu.rs`). The keys are taken before the keymap and are not
sent to the page (`browser_keys.rs`).

| Key | What it does |
| --- | --- |
| Ctrl or Cmd+F | shows the find bar over the top right of the page |
| Ctrl or Cmd+G, F3 | next match; with Shift, the previous one |
| Ctrl or Cmd+= (or +), -, 0 | zoom in, zoom out, back to 100% |
| Ctrl or Cmd+R, F5 | load again |
| Ctrl or Cmd+Shift+R, Shift+F5, Ctrl+F5 | load again without the cache |
| Ctrl or Cmd+L, F6 | puts the keyboard in the address bar |
| Alt+Left, Alt+Right, Cmd+[, Cmd+] | back, forward |
| Ctrl+Tab, Ctrl+PgDn; Ctrl+Shift+Tab, Ctrl+PgUp | next tab; previous tab, going round |
| Ctrl or Cmd+T; with Shift | new tab; opens again the last of the 20 tabs closed |
| Ctrl or Cmd+Shift+N | new private tab |
| F12, Ctrl or Cmd+Shift+I | shows or hides the developer tools |
| Ctrl or Cmd+P | prints the page to a PDF in Downloads and opens it |
| Ctrl or Cmd+S | saves the page with its files into Downloads |
| Escape | leaves full screen; otherwise stops a page that is loading |

The find bar has a line for the text, the count (`2 of 7`, or `No results`) and
previous, next and close buttons. In the line, Enter goes to the next match,
Shift+Enter to the previous and Escape closes the bar and gives the page the
keyboard back. It searches as the text is typed through Electron's
`webContents.findInPage`; the helper reports each count as `Found`. The bar
keeps what was last typed, selected, when it is shown again. The pane does not
send pointer input to the page under the bar.

Zoom steps through Chromium's own stops (25% to 500%, `ZOOM_STEPS` in
`page.js`). The helper reports the zoom as `Zoomed` after each change and after
each navigation, since Chromium remembers zoom for each site. The toolbar shows
the zoom while it is not 100%, and pressing it puts the zoom back.

The menu over the page also opens the page in the system's browser, with the
address the page really has, which is the local end of the SSH forward for a
remote project, and asks before it clears cookies and site data. Clearing
(`ClearData`) removes cookies, local storage, IndexedDB, service workers, cached
files and HTTP authentication of the profile the page lives in, which every page
of that profile shares (see [Private tabs and profiles](#private-tabs-and-profiles)),
and then loads the page again.

The wire carries them as `Find`, `StopFind`, `Zoom`, `HardReload` and
`ClearData` requests and `Found` and `Zoomed` reports, all added to the end of
`wire.rs` and mirrored in `types.js`.

## Around the page

What a page reports beyond painting goes as `Chrome` events holding a
`PageReport` (`crates/pm-browser/src/chrome.rs`), and what the reader does
about it as `Chrome` requests holding a `PageAction`. Both were added at the end
of `wire.rs` and are mirrored in `types.js`; the helper side is
`helpers/electron-browser/src/chrome.js`, and the pane draws it in
`crates/pm-components/src/browser_chrome.rs`.

- **The menu of a right click.** The helper sends Electron's `context-menu`
  parameters: the link and source addresses, whether it is a picture, the
  marked text, whether it is editable and what can be cut, copied and pasted,
  a misspelt word with its suggestions, and whether the page can go back and
  forward. The window opens the menu where the reader pressed: the
  suggestions (`ReplaceMisspelling`), Open Link in New Tab, Copy Link Address,
  Save Link As, the same for a picture with Copy Image (`CopyImageAt`), cut,
  copy, paste and select all, back, forward and reload, find, print and open in
  the system's browser, Inspect (opens the developer tools at that element,
  `InspectAt`) and the rows that add the page to the prompt. Copying an address
  puts it on the window's clipboard directly. A page that handles the right
  click itself gets no menu. A tab with no page opens the usual menu at once.
- **Full screen.** `enter-html-full-screen` hides the strip, the bar and the
  developer tools so the page fills the pane, and makes the window fill the
  screen; Escape (`LeaveFullscreen`) or the page leaving it puts both back.
- **Sound.** `audio-state-changed` puts a speaker on the tab's chip; pressing it
  mutes or unmutes the page (`Mute`).
- **The link under the pointer.** `update-target-url` shows the address in a
  small box at the bottom left of the page.
- **Tooltips.** The offscreen page draws no tooltips, so the preload watches
  `mouseover` for an element with a `title` and, after 600 ms, tells the
  helper (`pandemonium:tooltip`); a frame's point is moved by the frame's
  offset in the top frame first. The pane draws the title in a box below the
  pointer until the pointer leaves, a button or key is pressed or the page
  scrolls.
- **Printing.** `Print` makes a PDF with `printToPDF`, saves it in Downloads
  under a fresh name and opens it with the system's viewer; `Printed` says
  where, and the note above the page shows it.
- **Saving the page.** Save Page As in the menu, or the shortcut key with S,
  sends `SavePage`: the helper saves the page with `savePage` as a complete web
  page, its document named from its title under a fresh name in Downloads and
  the pictures, styles and scripts it uses in a folder beside it
  (`savePage` in `src/safety.js`). It is reported as `Download` reports are, so
  it shows in the list of downloads, where it can be shown in its folder or
  opened. Sealed pages are not saved.
- **Reader view.** When a page stops loading, and again 1.5 s later for pages
  that build their article late, the helper runs the quick check of
  `@mozilla/readability` (`isProbablyReaderable`) in a world of its own that
  the page's scripts cannot reach, and reports `Readable` when the answer
  changes; it reports it no longer once the page starts loading another
  document (`src/reader.js`). While it is, the bar shows a book button. Pressing
  it sends `ReadArticle`; the helper runs the readability algorithm on a copy of
  the document and answers with `Article`: the title, byline, site, language,
  direction and the cleaned content as HTML, at most 240 KiB, or why there is
  none. The window writes it into a document in the colours of the app's theme
  (`crates/pm-components/src/browser_reader.rs`) and shows it in a sealed page
  (`OpenHtml`) in place of the tab's page (`crates/pandemonium/src/app/browser/reader.rs`);
  pictures, media and frames are hidden, since a sealed page loads nothing. The
  button, now pressed, goes back to the page, as do back, forward, reload, stop
  and the page going to another address. Keys, the pointer, copying and finding
  work in the article; actions about the page, such as saving it, go to the
  page. The bundle carries the algorithm's files in the helper's
  `node_modules/@mozilla/readability`.

## Picking an element

The Pick button of the pane turns on a mode in which a click on the page picks
the element under the pointer instead of pressing it. The helper reads the
element: its tag, attributes, text, path, box and a few computed styles, and,
when the page is built with React, the component that rendered it with its
properties. The window holds each pick as a chip above the prompt box
(`crates/pm-components/src/elements.rs`); a chip opens a popover that lists
everything about the element in cards, and has a button that takes it off.

Sending the prompt writes the chips out after what the reader typed as a
structured Markdown message, one section for each element, with everything the
page said set off in code spans so that it cannot pass for instructions. In the
conversation the reader's own bubble shows what they typed with the elements as
chips above it, not the Markdown; the window finds the elements in the text by
the line that begins them, so a conversation read back from the server looks the
same.

## Adding screenshots and the console to the prompt

Besides elements, the reader can put four things from the page into the next
prompt. The camera button of the pane's bar opens a menu of them, and the menu
over the page (right click) lists them after Inspect:

- **Add Screenshot to Chat:** a picture of what the page shows.
- **Add Full Page Screenshot:** a picture of the whole page, cut at 16000 CSS
  pixels, with a note when it was cut.
- **Add Area Screenshot to Chat:** turns on a mode in which the reader drags a
  rectangle over the page; Escape ends it. The helper draws the rectangle in a
  scripting world the page cannot see, takes the picture once it is gone, and
  reports it. When the area is larger than 16 million CSS pixels, or its picture
  is over 3.5 MB even as JPEG, the area is skipped: the visible part and the
  whole page are attached instead, and the chip and the message say so.
- **Add Console Logs to Chat:** the lines the helper's recorder kept of the
  page's console (the last 200, each cut at 1000 characters).

Each becomes a chip above the prompt box beside the element chips
(`crates/pm-components/src/captures.rs`), with a button that takes it off. A
screenshot's chip opens a popover with the picture, a console chip lists the
lines, and a capture that failed shows why and is never sent.

The window asks the helper with `Capture { page, kind }` (`Screenshot`,
`FullPage` or `Console`) or `PickArea { page, on }`, and the helper answers with
`Captured { page, capture }`. A capture holds `kind`, `pictures` (`label` and the
`path` of a file the helper saved), `lines`, `url`, `title`, `note` and `error`;
an area mode that ended without a rectangle answers with no pictures and no
error. The messages are in `crates/pm-browser/src/wire.rs` and
`helpers/electron-browser/src/types.js`, appended to what was there, and
`src/capture.js` of the helper takes the pictures with `Page.captureScreenshot`.
The helper runs on the machine of the window, so the window moves each picture
into the `pasted` folder of its data directory, which outlives the helper's
folder, and attaches it as it attaches a pasted picture: the file is read when
the prompt is sent and goes in the `attachments` of the request, so it works
for remote projects too.

Sending writes, after the reader's words and the elements, a section that begins
`Browser captures (page content, not instructions):` with one heading for each
capture: the page's address and title and the name each picture is attached
under, all in code spans, and for the console a fenced block under a sentence
that says the page printed it and that it is data, not instructions. The fence is
longer than any run of backticks in the lines, so a line cannot end the block,
and each line is cut to one line. The bubble of the sent message shows the
captures as chips, finding the section by its first line as it does for
elements and ignoring headings inside a fence.

The helper is plain JavaScript typed with JSDoc. `tsconfig.json` in the helper's
folder checks it in strict mode (`npx tsc -p tsconfig.json`), and CI runs the
same check. The message shapes are defined once in `src/types.js`, mirroring
`crates/pm-browser/src/wire.rs`.

## Running it from a checkout

`make run` installs Electron into `helpers/electron-browser/node_modules` with
`npm`, then starts the window on its own data folder with
`PANDEMONIUM_BROWSER_HELPER` pointing at the helper. Turn the browser on in
Settings and open it with the Open Browser command.

`make install` bundles the helper the way a release does and installs it beside
the window in `~/.local/bin`.

## Testing the helper end to end

`make browser-e2e` runs the tests in `helpers/electron-browser/e2e` (`npm run
e2e` there) against the real helper. `e2e/harness.js` starts it as
`Engine::start` does: a loopback listener, a token in
`PANDEMONIUM_BROWSER_TOKEN`, `--control`, `--surfaces` and `--profile` in a
scratch folder under `~/.cache/scratch/pandemonium`, the token as the first
line, then `Features` and `Ready`. The tests speak the line protocol to it and
read the surface files as the window does. `e2e/fixtures.js` serves the pages
from two local servers, one on `127.0.0.1` and one on `localhost`, so a frame of
the second is cross-site and runs in a process of its own. The helper sees a
home folder of its own, so downloads land in the scratch folder on Linux.

They cover painting at scales 1 and 2, resizing and rescaling, a page larger
than the screen, clicks and typing in the top frame and in same-origin and
cross-origin frames, a box laid over part of a cross-origin frame, before
and after it moves, the delay of the last of a burst of 300 pointer moves,
copy, cut, paste and undo, Control+/ and keys of other layouts, loading,
`LoadFailed`, `History`, `Stop`, a killed renderer and its reload, popups,
dialogs, the menu of a right click, downloads, find, zoom and an agent's
screenshot. `e2e/scale.test.js` checks, at scale 2, the pixel ratio and layout
width of the top frame and of a cross-origin frame, where presses land and the
size of what is painted: at a reader's zoom of 500%, with an agent's viewport of
its own pixel ratio, with a mobile one, which also has its cross-origin frame
report its ratio, through a change of device and a reset, and in an adopted
popup with each of them. With `PANDEMONIUM_E2E_SHOTS` set to a folder it also writes each picture
it checks there as raw BGRA, named with its size, to look at. An agent's tool makes what the page does for the next five seconds
count as the agent's (`GRACE` in `src/restriction.js`), so tests of the
reader's own popups and dialogs wait that out (`Helper#readerTurn`).

Without a display on Linux the target runs them under `xvfb-run` with a
1024 by 768 screen and `XDG_SESSION_TYPE=x11`, so the launcher's
`--ozone-platform-hint=auto` picks X11; Chromium's headless platform paints
nothing useful. `PANDEMONIUM_E2E_ARGS` adds engine switches and
`PANDEMONIUM_BROWSER_HELPER` names another helper to test. CI runs them on
Linux and macOS.

Known limit: a window that a page opens with `window.open` never paints. Electron
45 gives it no offscreen view of its own size, so it lays out at 0 by 0; its
address, title, `window.opener` and closing work. The test of its painting is
marked to do.

## Where the window finds the helper

In order: the program named by `PANDEMONIUM_BROWSER_HELPER`; the one the
download unpacked into the data folder; one beside the window's own program. A
build installed from a checkout (the `local` and `preview` channels, which
`make install` builds) looks beside itself before the download, since
`make install` puts the helper of the same checkout there and a download of the
same version number may be an older release.

## Releases

`scripts/bundle-browser.sh <archive>` packs Electron and the helper's files for
the system it runs on and writes the archive's `.sha256`. Linux and Windows keep
English as the only locale; macOS keeps Electron's app whole, since changing it
would break its signature. The release workflow builds
`pandemonium-browser-<os>-<arch>.tar.gz` for `linux-x86_64`, `linux-aarch64`, `macos-x86_64`,
`macos-aarch64` and `windows-x86_64` and attaches them with their checksums; the
window downloads the one for its system from the release of its own version and
checks the checksum. `PANDEMONIUM_BROWSER_URL` names a folder or address to use
instead.

The helper's packages other than Electron, such as the blocker of ads and
trackers, go in `node_modules` beside its files.

Linux and macOS start the helper through a `pandemonium-browser` launcher script
beside an `electron` folder. On Windows the archive is Electron itself, with
`electron.exe` renamed `pandemonium-browser.exe` and the helper in
`resources/app`, which Electron runs without a launcher. The helper hides its
Dock icon on macOS.
