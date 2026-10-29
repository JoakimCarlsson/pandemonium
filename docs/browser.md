# The embedded browser

The browser pane shows web pages beside an agent's work, such as the dev
server it is building. It is opt-in: Settings, under General, turns it on and
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

The window then opens the browser on the address, or sends an open browser to
it, over the same SSH forward as any other page of that machine. It happens
once for each address a command prints, only while the embedded browser is on
and installed, and Settings, under General, can turn it off. A shell in a dev
container is not seen, since its ports are the container's.

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
