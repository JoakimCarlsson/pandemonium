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
  the tools run in the helper of the window that announced the browser. See
  [protocol.md](protocol.md#browser-tools).

## How the window and the helper talk

The window starts the helper with `--control <socket> --surfaces <folder>
--profile <folder>` and connects over the Unix socket. Requests and reports
are one JSON object to a line; `crates/pm-browser/src/wire.rs` defines them.

Each page paints into two files in the surfaces folder, written in turn. A
file starts with a counter that is odd while the helper fills it and even
otherwise; the window copies the pixels out only when it sees the same even
number before and after. A `Surface` report names the files and a `Painted`
report says which is newest.

The helper runs each page as an offscreen Electron window and replays the
window's pointer, scroll and key requests as input events. The picker and the
agent tools use the page's DevTools protocol through Electron's debugger. The
developer tools are Electron's own, painted into a second page.

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
once for each address a command prints. A browser that is open on another
page is left there when a different server starts, since the reader or an agent
is using it; it goes to a server's page when it shows nothing, an error or a page
of that same server, which it loads again where it is. It happens only while the embedded browser is on
and installed, and Settings, under Browser, can turn it off. A shell in a dev
container is not seen, since its ports are the container's.

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
drag (`browser_drag`, by element or point; HTML drag and drop through
`Input.setInterceptDrags`, other drags as plain mouse events), fill several
fields at once (`browser_fill_form`, which runs the same code as `browser_type`,
`browser_select_option` and a click on a box, with the same refusal messages and
a stop at the first failure), outline an element for the person watching
(`browser_highlight`, a fixed overlay that removes itself), measure one
(`browser_get_bounding_box`), take a picture of the whole page or one element
(`browser_screenshot` with `full_page`, `ref` or `selector`; it uses
`captureBeyondViewport`, and a full-page picture is cut at 16000 pixels) and set
the viewport size (`browser_resize`).

`browser_resize` uses `Emulation.setDeviceMetricsOverride`, so the size stays
through navigation. It does not fight the pane: the helper applies the override
again after each `Resize` request of the pane, which still sizes the window and
the painted surface, so the page is shown at the emulated size at the top left
of the pane and the rest of the pane is empty. `reset` gives the size back to
the pane.

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
- `prompt` is cancelled at once, without a box: the page gets `null`.
- `beforeunload` emits `will-prevent-unload`; if nobody calls `preventDefault`,
  the navigation is cancelled silently (`did-fail-load` with `ERR_ABORTED`).
- The DevTools protocol reports `alert` and `confirm` with
  `Page.javascriptDialogOpening` and answers them with
  `Page.handleJavaScriptDialog`.

So the helper replaces `dialog.showMessageBox` for page dialogs with a promise
that the helper settles itself (nothing native opens), records an open dialog
from the protocol, and has `browser_handle_dialog` answer it. While a dialog is
open every tool that talks to the page (everything but `browser_console` and
`browser_network`) returns at once with the dialog described instead of
hanging; a tool that makes the page open one returns as soon as it does. Every
answer ends with a "Dialogs" list of what is open and what closed since the agent
was last told. A dialog nobody answers is dismissed after 60 seconds. While a
tool is running, a `beforeunload` prompt is overridden so the tool can leave the
page; at other times Electron's cancel stands, and the agent is told.

Known limits: `prompt` cannot be answered, since Electron cancels it before the
protocol sees it, so `prompt_text` has no effect. The pane draws no dialog, so a
person looking at the pane sees a stopped page until an agent answers or the
60 seconds pass. The patch of `dialog.showMessageBox` relies on Electron calling
it through the module object, as it does at the time of writing; if it did not
take, the native box would open and the protocol could still answer it.

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
  again. Putting text on the clipboard in answer to a click is still allowed.
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
  before the extension when the name is taken, so nothing is overwritten. The
  pane says what was saved (`Download`, as started, finished or failed) and
  nothing is opened.
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

## Back and forward buttons

The mouse's back and forward buttons go through the history of the page while
the pointer is over it, or it holds the keyboard and is on screen. Anywhere else
they move through the pages of the window, as before.

## The address bar

What is typed in the address bar becomes an address in
`crates/pm-components/src/browser_address.rs`. Text with a scheme (`https://`,
`about:`, `data:`) is loaded as it is. A host gets a scheme: `http://` for
`localhost`, loopback addresses, `*.localhost` and `:3000`, which means
`localhost:3000`, and `https://` for anything with a dot or written as
`name:port`. Everything else is searched for: text with spaces, and one word
without a dot that is not a local host, such as `react`. The search is
DuckDuckGo (`SEARCH_URL`). The engine is fixed in code; there is no setting for
it yet, since the address is made in `pm-components`, which does not see the
window's settings. Nothing typed loads `about:blank`.

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
files and HTTP authentication of the helper's one session, which every page of the
browser shares, and then loads the page again.

The wire carries them as `Find`, `StopFind`, `Zoom`, `HardReload` and
`ClearData` requests and `Found` and `Zoomed` reports, all added to the end of
`wire.rs` and mirrored in `types.js`.

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

## Where the window finds the helper

In order: the program named by `PANDEMONIUM_BROWSER_HELPER`; the one the
download unpacked into the data folder; one beside the window's own program.

## Releases

`scripts/bundle-browser.sh <archive>` packs Electron, with English as its only
locale, and the helper's files into `pandemonium-browser-linux-x86_64.tar.gz`
and writes its `.sha256`. The release workflow attaches both to the release; the
window downloads them from the release of its own version and checks the
checksum. `PANDEMONIUM_BROWSER_URL` names a folder or address to use instead.

The helper is built for Linux on x86-64 only so far. Other systems need a
launcher for their Electron build and the window's control connection on
Windows.
