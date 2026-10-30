//! Pixels shared between the window and the helper through a file both map, written by one and read by the other without a lock.
//!
//! A file starts with a counter that is odd while the writer is filling it
//! and even otherwise, and counts up by two with each finished fill; a reader
//! that sees the same even number before and after copying out the pixels has
//! a whole picture.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering, fence};

use memmap2::{Mmap, MmapMut};

/// How many bytes of a file come before the pixels.
const HEADER: usize = 16;

/// The bytes `width` by `height` pixels take, four to a pixel.
fn pixel_bytes(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// The counter at the start of a mapped file.
fn counter(base: *const u8) -> &'static AtomicU64 {
    // SAFETY: the mapping is page-aligned and at least HEADER bytes long, and lives as long as the
    // reference is used, which callers keep within one call.
    unsafe { &*base.cast::<AtomicU64>() }
}

/// A file the helper paints into.
pub struct SurfaceWriter {
    /// The mapped file.
    map: MmapMut,
    /// How many pixels it holds.
    pixels: usize,
}

impl SurfaceWriter {
    /// Creates the file at `path` with room for `width` by `height` pixels and maps it.
    pub fn create(path: &Path, width: u32, height: u32) -> io::Result<Self> {
        let pixels = pixel_bytes(width, height);
        let file: File = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len((HEADER + pixels) as u64)?;
        // SAFETY: the file is private to the window and the helper, which follow the counter protocol.
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self { map, pixels })
    }

    /// Writes `bgra`, which is as long as the surface, as RGBA pixels.
    pub fn write(&mut self, bgra: &[u8]) {
        if bgra.len() < self.pixels {
            return;
        }
        let base = self.map.as_mut_ptr();
        let sequence = counter(base);
        let before = sequence.load(Ordering::Relaxed);
        sequence.store(before + 1, Ordering::Relaxed);
        fence(Ordering::Release);
        let target = &mut self.map[HEADER..HEADER + self.pixels];
        for (out, pixel) in target
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(bgra.as_chunks::<4>().0)
        {
            out[0] = pixel[2];
            out[1] = pixel[1];
            out[2] = pixel[0];
            out[3] = pixel[3];
        }
        sequence.store(before + 2, Ordering::Release);
    }
}

/// A file the window reads the pictures of a page from.
pub struct SurfaceReader {
    /// The mapped file.
    map: Mmap,
    /// How many bytes of pixels it holds.
    pixels: usize,
}

impl SurfaceReader {
    /// Maps the file at `path`, which holds `width` by `height` pixels.
    pub fn open(path: &Path, width: u32, height: u32) -> io::Result<Self> {
        let file = File::open(path)?;
        // SAFETY: see `SurfaceWriter::create`.
        let map = unsafe { Mmap::map(&file)? };
        let pixels = pixel_bytes(width, height);
        if map.len() < HEADER + pixels {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "surface too small",
            ));
        }
        Ok(Self { map, pixels })
    }

    /// The pixels as RGBA, or nothing when the writer was filling the file while they were copied.
    pub fn read(&self) -> Option<Vec<u8>> {
        let sequence = counter(self.map.as_ptr());
        let before = sequence.load(Ordering::Acquire);
        if before % 2 == 1 {
            return None;
        }
        let mut copy = vec![0; self.pixels];
        // SAFETY: both ranges are inside their allocations; a concurrent write is detected below
        // and the copy thrown away.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.map.as_ptr().add(HEADER),
                copy.as_mut_ptr(),
                self.pixels,
            );
        }
        fence(Ordering::Acquire);
        (sequence.load(Ordering::Relaxed) == before).then_some(copy)
    }
}
