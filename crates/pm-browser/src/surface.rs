//! Pixels shared between the window and the helper through a file both map, written by one and read by the other without a lock.
//!
//! A file starts with a counter that is odd while the writer is filling it
//! and even otherwise, and counts up by two with each finished fill; a reader
//! that sees the same even number before and after copying out the pixels has
//! a whole picture. A counter still at zero means the file was never filled.
//!
//! The four bytes after the counter say how the pixels are laid out: `BGRA`
//! when the writer stores them as the engine paints them, so it does no work
//! per pixel, and anything else for RGBA. The reader turns BGRA into RGBA
//! while copying, into a buffer it is handed and can reuse.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering, fence};

use memmap2::{Mmap, MmapMut};

/// How many bytes of a file come before the pixels.
const HEADER: usize = 16;

/// Where the tag of the pixel layout starts.
const FORMAT_AT: usize = 8;

/// The tag of pixels stored as BGRA.
const BGRA: [u8; 4] = *b"BGRA";

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

/// Swaps the blue and red bytes of every pixel of `pixels`, turning BGRA into RGBA and back.
fn swap_red_blue(pixels: &mut [u8]) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
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
        let mut map = unsafe { MmapMut::map_mut(&file)? };
        map[FORMAT_AT..FORMAT_AT + BGRA.len()].copy_from_slice(&BGRA);
        Ok(Self { map, pixels })
    }

    /// Writes `bgra`, which is as long as the surface, as it is; nothing is written when it is shorter.
    pub fn write(&mut self, bgra: &[u8]) {
        if bgra.len() < self.pixels {
            return;
        }
        let base = self.map.as_mut_ptr();
        let sequence = counter(base);
        let before = sequence.load(Ordering::Relaxed);
        sequence.store(before + 1, Ordering::Relaxed);
        fence(Ordering::Release);
        self.map[HEADER..HEADER + self.pixels].copy_from_slice(&bgra[..self.pixels]);
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

    /// How many bytes of pixels a picture in the file takes, which is how long a buffer for [`SurfaceReader::read_into`] must be.
    pub fn len(&self) -> usize {
        self.pixels
    }

    /// Whether a picture in the file has no pixels.
    pub fn is_empty(&self) -> bool {
        self.pixels == 0
    }

    /// Whether the writer stores the pixels as BGRA.
    fn bgra(&self) -> bool {
        self.map[FORMAT_AT..FORMAT_AT + BGRA.len()] == BGRA
    }

    /// The pixels as RGBA, or nothing when the file was never filled or the writer was filling it while they were copied.
    pub fn read(&self) -> Option<Vec<u8>> {
        let mut copy = vec![0; self.pixels];
        self.read_into(&mut copy).then_some(copy)
    }

    /// Copies the pixels as RGBA into `out`, which must be [`SurfaceReader::len`] bytes long, and says whether they
    /// are a whole picture; on `false`, which also comes of a file never filled or an `out` of another length, what
    /// `out` holds is to be thrown away.
    pub fn read_into(&self, out: &mut [u8]) -> bool {
        if out.len() != self.pixels {
            return false;
        }
        let sequence = counter(self.map.as_ptr());
        let before = sequence.load(Ordering::Acquire);
        if before == 0 || before % 2 == 1 {
            return false;
        }
        // SAFETY: both ranges are inside their allocations; a concurrent write is detected below
        // and the copy thrown away.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.map.as_ptr().add(HEADER),
                out.as_mut_ptr(),
                self.pixels,
            );
        }
        fence(Ordering::Acquire);
        if sequence.load(Ordering::Relaxed) != before {
            return false;
        }
        if self.bgra() {
            swap_red_blue(out);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch folder in the repository's ignored `build/` for the test `name`.
    fn scratch(name: &str) -> std::path::PathBuf {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/pm-browser-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("scratch folder");
        folder
    }

    /// A file written as BGRA reads back as RGBA, and an unfilled one reads as nothing.
    #[test]
    fn bgra_reads_back_as_rgba() {
        let folder = scratch("bgra");
        let path = folder.join("page.rgba");
        let mut writer = SurfaceWriter::create(&path, 2, 1).expect("create");
        let reader = SurfaceReader::open(&path, 2, 1).expect("open");
        let mut out = vec![0; reader.len()];
        assert!(!reader.read_into(&mut out));
        writer.write(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(reader.read_into(&mut out));
        assert_eq!(out, [3, 2, 1, 4, 7, 6, 5, 8]);
        assert!(!reader.read_into(&mut [0; 4]));
        let _ = std::fs::remove_dir_all(folder);
    }

    /// A file without the tag, as an older helper writes it, is read as RGBA.
    #[test]
    fn untagged_reads_as_rgba() {
        let folder = scratch("rgba");
        let path = folder.join("page.rgba");
        let mut bytes = vec![0; HEADER + 4];
        bytes[0] = 2;
        bytes[HEADER..].copy_from_slice(&[9, 8, 7, 6]);
        std::fs::write(&path, bytes).expect("write");
        let reader = SurfaceReader::open(&path, 1, 1).expect("open");
        assert_eq!(reader.read(), Some(vec![9, 8, 7, 6]));
        let _ = std::fs::remove_dir_all(folder);
    }
}
