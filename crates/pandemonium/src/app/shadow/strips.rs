//! The shadow of a Wayland window drawn on four strips of surface laid under its edges.
//!
//! The strips are subsurfaces of the window, on a connection of our own to the compositor that shares the window's display. They reach out past the window, so the shadow they hold is outside the window geometry the compositor places, snaps and tiles, and they take no pointer input, so presses on the shadow go to what is below. They are synchronized with the window, so a change to them shows with the next frame the window presents.

use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::ptr::NonNull;

use pm_components::{FRAME_RADIUS, SHADOW_MARGIN, WINDOW_SHADOW};
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_subcompositor::WlSubcompositor;
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;

/// How far the shadow of a quad fades, as a share of its blur radius, as the renderer draws it.
const SOFTNESS: f32 = 1.3;

/// A rectangle in logical pixels, from the top left corner of the window.
#[derive(Clone, Copy, PartialEq)]
struct Rect {
    /// The left edge.
    x: i32,
    /// The top edge.
    y: i32,
    /// The width.
    width: i32,
    /// The height.
    height: i32,
}

/// What the strips were last fitted to.
#[derive(Clone, Copy, PartialEq)]
struct Fit {
    /// The size of the window in logical pixels.
    size: (u32, u32),
    /// The scale of the window, as bits of the float.
    scale: u64,
    /// Whether the window floated, and so had a shadow.
    floating: bool,
}

/// What the events of the connection change: the buffers the compositor still holds.
#[derive(Default)]
pub(crate) struct Held {
    /// The buffers attached to a strip that the compositor has not released yet.
    buffers: Vec<WlBuffer>,
}

/// One strip of surface under an edge of the window.
struct Strip {
    /// The surface the strip shows.
    surface: WlSurface,
    /// The role that places the surface beside the window's.
    subsurface: WlSubsurface,
    /// What scales the buffer of the strip to its size, when the compositor offers it.
    viewport: Option<WpViewport>,
}

/// The four strips under the edges of one Wayland window and the connection they are made on.
pub(crate) struct Strips {
    /// Our connection to the compositor, on the window's display.
    connection: Connection,
    /// The queue the events of our objects arrive on.
    queue: EventQueue<Held>,
    /// What those events change.
    held: Held,
    /// The shared memory buffers are made in.
    shm: WlShm,
    /// The strips: the top, bottom, left and right one.
    strips: [Strip; 4],
    /// What the strips were last fitted to, if anything.
    fitted: Option<Fit>,
}

impl Strips {
    /// Strips under the Wayland surface `surface` on the display `display`, both as the window hands them out; `None` when the compositor lacks what they need.
    ///
    /// The window's event loop must keep `display` open, and the window must keep `surface` alive, for as long as the strips live: winit does both, as the strips are dropped before their window and every window before the event loop.
    pub(crate) fn open(display: NonNull<c_void>, surface: NonNull<c_void>) -> Option<Self> {
        let connection = share(display);
        let parent = beside(&connection, surface)?;
        let (globals, mut queue) = registry_queue_init::<Held>(&connection).ok()?;
        let handle = queue.handle();
        let compositor: WlCompositor = globals.bind(&handle, 4..=6, ()).ok()?;
        let subcompositor: WlSubcompositor = globals.bind(&handle, 1..=1, ()).ok()?;
        let shm: WlShm = globals.bind(&handle, 1..=1, ()).ok()?;
        let viewporter: Option<WpViewporter> = globals.bind(&handle, 1..=1, ()).ok();
        let strips = [(); 4].map(|()| {
            strip(
                &compositor,
                &subcompositor,
                viewporter.as_ref(),
                &parent,
                &handle,
            )
        });
        if let Some(viewporter) = viewporter {
            viewporter.destroy();
        }
        subcompositor.destroy();
        let mut held = Held::default();
        let _ = queue.roundtrip(&mut held);
        Some(Self {
            connection,
            queue,
            held,
            shm,
            strips,
            fitted: None,
        })
    }

    /// Fits the strips to a window `size` logical pixels across at `scale`: they show its shadow while it is `floating` and nothing while it is not. Does nothing when that is what they last showed.
    pub(crate) fn fit(&mut self, size: (u32, u32), scale: f64, floating: bool) {
        let fit = Fit {
            size,
            scale: scale.to_bits(),
            floating,
        };
        if self.fitted == Some(fit) {
            return;
        }
        self.fitted = Some(fit);
        let _ = self.queue.dispatch_pending(&mut self.held);
        let rects = rects(size);
        for (strip, rect) in self.strips.iter().zip(rects) {
            let buffer = match rect.filter(|_| floating) {
                Some(rect) => self.paint(strip, rect, size, scale),
                None => None,
            };
            strip.surface.attach(buffer.as_ref(), 0, 0);
            strip.surface.commit();
            if let Some(buffer) = buffer {
                self.held.buffers.push(buffer);
            }
        }
        let _ = self.connection.flush();
    }

    /// Places `strip` at `rect` beside a window `size` logical pixels across and draws its part of the shadow into a new buffer at `scale`, which it returns; `None` when no buffer could be made.
    fn paint(&self, strip: &Strip, rect: Rect, size: (u32, u32), scale: f64) -> Option<WlBuffer> {
        strip.subsurface.set_position(rect.x, rect.y);
        let (width, height) = match strip.viewport.as_ref() {
            Some(viewport) => {
                viewport.set_destination(rect.width, rect.height);
                (
                    (f64::from(rect.width) * scale).ceil() as i32,
                    (f64::from(rect.height) * scale).ceil() as i32,
                )
            }
            None => {
                let whole = scale.ceil().max(1.0) as i32;
                strip.surface.set_buffer_scale(whole);
                (rect.width * whole, rect.height * whole)
            }
        };
        let pixels = shade(rect, size, (width as usize, height as usize));
        let buffer = self.buffer(&pixels, width, height)?;
        strip.surface.damage_buffer(0, 0, width, height);
        Some(buffer)
    }

    /// A buffer `width` by `height` pixels holding `pixels`, premultiplied ARGB in the byte order of the machine; `None` when its memory could not be had.
    fn buffer(&self, pixels: &[u8], width: i32, height: i32) -> Option<WlBuffer> {
        let fd = memfd_create("pandemonium-shadow", MemfdFlags::CLOEXEC).ok()?;
        let mut file = File::from(fd);
        file.write_all(pixels).ok()?;
        let handle = self.queue.handle();
        let pool = self
            .shm
            .create_pool(file.as_fd(), pixels.len() as i32, &handle, ());
        let buffer = pool.create_buffer(0, width, height, width * 4, Format::Argb8888, &handle, ());
        pool.destroy();
        Some(buffer)
    }
}

impl Drop for Strips {
    /// Takes the strips and every buffer still held off the compositor.
    fn drop(&mut self) {
        for strip in &self.strips {
            if let Some(viewport) = strip.viewport.as_ref() {
                viewport.destroy();
            }
            strip.subsurface.destroy();
            strip.surface.destroy();
        }
        for buffer in self.held.buffers.drain(..) {
            buffer.destroy();
        }
        let _ = self.connection.flush();
    }
}

/// A strip under the surface `parent`, below it, that takes no pointer input and shows nothing yet.
fn strip(
    compositor: &WlCompositor,
    subcompositor: &WlSubcompositor,
    viewporter: Option<&WpViewporter>,
    parent: &WlSurface,
    handle: &QueueHandle<Held>,
) -> Strip {
    let surface = compositor.create_surface(handle, ());
    let subsurface = subcompositor.get_subsurface(&surface, parent, handle, ());
    subsurface.place_below(parent);
    subsurface.set_sync();
    let region = compositor.create_region(handle, ());
    surface.set_input_region(Some(&region));
    region.destroy();
    let viewport = viewporter.map(|viewporter| viewporter.get_viewport(&surface, handle, ()));
    surface.commit();
    Strip {
        surface,
        subsurface,
        viewport,
    }
}

/// A connection of our own on the Wayland display `display` the window is on, with its own queue of events.
///
/// The display must stay open for as long as the connection lives, which it does: winit keeps it open while its event loop runs, and the event loop outlives every window and so every connection made for one.
fn share(display: NonNull<c_void>) -> Connection {
    let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
    Connection::from_backend(backend)
}

/// The window's `wl_surface` at `surface`, as an object of `connection`; `None` when the pointer is not a surface.
///
/// The surface must stay alive for as long as the object is used, which it does: it is only used while the window is open, to lay the strips beside it.
fn beside(connection: &Connection, surface: NonNull<c_void>) -> Option<WlSurface> {
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr().cast()) }.ok()?;
    WlSurface::from_id(connection, id).ok()
}

/// Where the top, bottom, left and right strip go around a window `size` logical pixels across: each reaches the width of the shadow out past the window and the radius of its corners in under it, so the shadow shows past the rounded corners; `None` for a strip the window is too small to have.
fn rects(size: (u32, u32)) -> [Option<Rect>; 4] {
    let margin = SHADOW_MARGIN.ceil() as i32;
    let radius = FRAME_RADIUS.ceil() as i32;
    let (width, height) = (size.0 as i32, size.1 as i32);
    let across = width + 2 * margin;
    let deep = margin + radius;
    let side = height - 2 * radius;
    [
        Rect {
            x: -margin,
            y: -margin,
            width: across,
            height: deep,
        },
        Rect {
            x: -margin,
            y: height - radius,
            width: across,
            height: deep,
        },
        Rect {
            x: -margin,
            y: radius,
            width: deep,
            height: side,
        },
        Rect {
            x: width - radius,
            y: radius,
            width: deep,
            height: side,
        },
    ]
    .map(|rect| (rect.width > 0 && rect.height > 0 && width > 0).then_some(rect))
}

/// The pixels of the strip at `rect` beside a window `size` logical pixels across, drawn `pixels` wide and tall: black, as opaque as the shadow of the window is there, premultiplied ARGB in the byte order of the machine.
///
/// Where the window's edge runs straight the shadow only changes across it, so one column, or one row, is worked out and copied along it.
fn shade(rect: Rect, size: (u32, u32), pixels: (usize, usize)) -> Vec<u8> {
    let (columns, rows) = pixels;
    let frame = (size.0 as f32, size.1 as f32);
    let across = rect.width as f32 / columns as f32;
    let down = rect.height as f32 / rows as f32;
    let x_at = |column: usize| rect.x as f32 + (column as f32 + 0.5) * across;
    let y_at = |row: usize| rect.y as f32 + (row as f32 + 0.5) * down;
    let (left, right) = straight(frame.0, |layer| layer.x);
    let (top, bottom) = straight(frame.1, |layer| layer.y);
    let straight_column = (0..columns).find(|&column| (left..=right).contains(&x_at(column)));
    let straight_row = (0..rows).find(|&row| (top..=bottom).contains(&y_at(row)));
    let column_alpha: Vec<u8> = straight_column.map_or_else(Vec::new, |column| {
        (0..rows)
            .map(|row| opacity(x_at(column), y_at(row), frame))
            .collect()
    });
    let row_alpha: Vec<u8> = straight_row.map_or_else(Vec::new, |row| {
        (0..columns)
            .map(|column| opacity(x_at(column), y_at(row), frame))
            .collect()
    });
    let mut out = vec![0; columns * rows * 4];
    for row in 0..rows {
        let y = y_at(row);
        let in_row = (top..=bottom).contains(&y);
        for column in 0..columns {
            let x = x_at(column);
            let alpha = if !column_alpha.is_empty() && (left..=right).contains(&x) {
                column_alpha[row]
            } else if !row_alpha.is_empty() && in_row {
                row_alpha[column]
            } else {
                opacity(x, y, frame)
            };
            out[(row * columns + column) * 4..][..4].copy_from_slice(&argb(alpha));
        }
    }
    out
}

/// Where along an edge of the window `length` logical pixels long the shadow of every layer runs straight, past the rounded corners, with `offset` the shift of a layer along that edge.
fn straight(length: f32, offset: impl Fn(&pm_style::decoration::ShadowLayer) -> f32) -> (f32, f32) {
    let layers = || WINDOW_SHADOW.iter().filter(|layer| !layer.inset);
    let most = layers().map(&offset).fold(f32::MIN, f32::max);
    let least = layers().map(&offset).fold(f32::MAX, f32::min);
    (most + FRAME_RADIUS, length + least - FRAME_RADIUS)
}

/// How opaque the shadow of a window `frame` logical pixels across is at the logical point `x`, `y`, from 0 to 255: every layer of it laid over the others, each the rounded frame moved, grown and faded at its edge as the renderer draws it.
fn opacity(x: f32, y: f32, frame: (f32, f32)) -> u8 {
    let clear = WINDOW_SHADOW
        .iter()
        .filter(|layer| !layer.inset)
        .fold(1.0, |clear, layer| {
            let radius = (FRAME_RADIUS + layer.spread).max(0.0);
            let distance = rounded_distance(
                x - layer.x,
                y - layer.y,
                (-layer.spread, -layer.spread),
                (frame.0 + layer.spread, frame.1 + layer.spread),
                radius,
            );
            let softness = (layer.blur * SOFTNESS).max(f32::EPSILON);
            let coverage = 1.0 - smoothstep(-softness, softness, distance);
            clear * (1.0 - layer.alpha * coverage)
        });
    ((1.0 - clear) * 255.0).round().clamp(0.0, 255.0) as u8
}

/// How far the point `x`, `y` is outside the rectangle from `from` to `to` with corners rounded by `radius`; negative inside.
fn rounded_distance(x: f32, y: f32, from: (f32, f32), to: (f32, f32), radius: f32) -> f32 {
    let half = ((to.0 - from.0) * 0.5, (to.1 - from.1) * 0.5);
    let centre = (from.0 + half.0, from.1 + half.1);
    let radius = radius.min(half.0).min(half.1);
    let qx = (x - centre.0).abs() - half.0 + radius;
    let qy = (y - centre.1).abs() - half.1 + radius;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - radius
}

/// Hermite interpolation of `value` from 0 at `low` to 1 at `high`.
fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The bytes of a black pixel `alpha` opaque, premultiplied ARGB in the byte order of the machine.
fn argb(alpha: u8) -> [u8; 4] {
    (u32::from(alpha) << 24).to_ne_bytes()
}

impl Dispatch<WlRegistry, GlobalListContents> for Held {
    /// Ignores globals coming and going: the strips bind what they need once.
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for Held {
    /// Destroys a buffer once the compositor releases it, as a newer one has replaced it.
    fn event(
        held: &mut Self,
        buffer: &WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, wl_buffer::Event::Release) {
            held.buffers.retain(|kept| kept != buffer);
            buffer.destroy();
        }
    }
}

delegate_noop!(Held: ignore WlCompositor);
delegate_noop!(Held: ignore WlSubcompositor);
delegate_noop!(Held: ignore WlSubsurface);
delegate_noop!(Held: ignore WlSurface);
delegate_noop!(Held: ignore WlRegion);
delegate_noop!(Held: ignore WlShm);
delegate_noop!(Held: ignore WlShmPool);
delegate_noop!(Held: ignore WpViewporter);
delegate_noop!(Held: ignore WpViewport);
