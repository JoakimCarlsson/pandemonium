//! Pixels shared between the window and the helper through a file both map, written by one and read by the other without a lock.
//!
//! A file starts with a counter that is odd while the writer is filling it
//! and even otherwise, and counts up by two with each finished fill; a reader
//! that sees the same even number before and after copying out the pixels has
//! a whole picture. A counter still at zero means the file was never filled.
//!
//! The four bytes after the counter say how the pixels are laid out: `BGRA`
//! when the writer stores them as the engine paints them, so it does no work
//! per pixel, and anything else for RGBA. The reader copies them as they are,
//! whole or only a rectangle, into a buffer it is handed and can reuse, and
//! says which layout they have; the renderer takes either without reordering.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering, fence};

use memmap2::{Mmap, MmapMut};

use crate::frame::Region;

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
    /// The width and height of a picture in it.
    size: (u32, u32),
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
        Ok(Self {
            map,
            pixels,
            size: (width, height),
        })
    }

    /// How many bytes of pixels a picture in the file takes, which is how long a buffer for [`SurfaceReader::read_into`] must be.
    pub fn len(&self) -> usize {
        self.pixels
    }

    /// Whether a picture in the file has no pixels.
    pub fn is_empty(&self) -> bool {
        self.pixels == 0
    }

    /// Whether the writer stores the pixels as BGRA rather than RGBA.
    pub fn bgra(&self) -> bool {
        self.map[FORMAT_AT..FORMAT_AT + BGRA.len()] == BGRA
    }

    /// The pixels as they are stored (see [`SurfaceReader::bgra`]), or nothing when the file was never filled or the writer was filling it while they were copied.
    pub fn read(&self) -> Option<Vec<u8>> {
        let mut copy = vec![0; self.pixels];
        self.read_into(&mut copy).then_some(copy)
    }

    /// Copies the pixels as they are stored into `out`, which must be [`SurfaceReader::len`] bytes long, and says
    /// whether they are a whole picture; on `false`, which also comes of a file never filled or an `out` of another
    /// length, what `out` holds is to be thrown away.
    pub fn read_into(&self, out: &mut [u8]) -> bool {
        self.read_region_into(out, Region::whole(self.size.0, self.size.1))
    }

    /// Copies only the pixels inside `region` into the same place of `out`, which must be [`SurfaceReader::len`]
    /// bytes long and already hold the rest of the picture, and says whether `out` then holds a whole picture, as
    /// [`SurfaceReader::read_into`] does. A region reaching past the picture is cut to it.
    pub fn read_region_into(&self, out: &mut [u8], region: Region) -> bool {
        if out.len() != self.pixels {
            return false;
        }
        let sequence = counter(self.map.as_ptr());
        let before = sequence.load(Ordering::Acquire);
        if before == 0 || before % 2 == 1 {
            return false;
        }
        let (width, height) = self.size;
        let region = Region::reported(
            [region.x, region.y, region.width, region.height],
            width,
            height,
        );
        let stride = width as usize * 4;
        let span = region.width as usize * 4;
        let first = region.x as usize * 4;
        let base = self.map.as_ptr();
        for row in region.y as usize..(region.y + region.height) as usize {
            let at = row * stride + first;
            // SAFETY: the row's span lies inside both the picture in the map and `out`, which are `pixels` bytes
            // long; a concurrent write is detected below and the copy thrown away.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    base.add(HEADER + at),
                    out.as_mut_ptr().add(at),
                    span,
                );
            }
        }
        fence(Ordering::Acquire);
        sequence.load(Ordering::Relaxed) == before
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

    /// A file written as BGRA reads back as it was written and says so, and an unfilled one reads as nothing.
    #[test]
    fn bgra_reads_back_as_written() {
        let folder = scratch("bgra");
        let path = folder.join("page.rgba");
        let mut writer = SurfaceWriter::create(&path, 2, 1).expect("create");
        let reader = SurfaceReader::open(&path, 2, 1).expect("open");
        let mut out = vec![0; reader.len()];
        assert!(!reader.read_into(&mut out));
        writer.write(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(reader.read_into(&mut out));
        assert!(reader.bgra());
        assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8]);
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
        assert!(!reader.bgra());
        assert_eq!(reader.read(), Some(vec![9, 8, 7, 6]));
        let _ = std::fs::remove_dir_all(folder);
    }

    /// A region copies only its own pixels and leaves the rest of the buffer as it was.
    #[test]
    fn a_region_copies_only_its_pixels() {
        let folder = scratch("region");
        let path = folder.join("page.rgba");
        let mut writer = SurfaceWriter::create(&path, 3, 2).expect("create");
        let reader = SurfaceReader::open(&path, 3, 2).expect("open");
        writer.write(&(1..=24).collect::<Vec<u8>>());
        let mut out = vec![0; reader.len()];
        let corner = Region {
            x: 1,
            y: 1,
            width: 9,
            height: 9,
        };
        assert!(reader.read_region_into(&mut out, corner));
        let expected: Vec<u8> = (1..=24)
            .map(|byte| if byte > 16 { byte } else { 0 })
            .collect();
        assert_eq!(out, expected);
        let _ = std::fs::remove_dir_all(folder);
    }

    /// The time copying a picture out of a file takes, whole and swapped to RGBA as the window once did, whole as it
    /// is stored, and only a caret's rectangle. Run with
    /// `cargo test -p pm-browser --release -- --ignored --nocapture surface_read_cost`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn surface_read_cost() {
        let folder = scratch("cost");
        for (name, width, height) in [("1080p", 1920u32, 1080u32), ("4K", 3840, 2160)] {
            let path = folder.join(format!("{name}.rgba"));
            let mut writer = SurfaceWriter::create(&path, width, height).expect("create");
            let reader = SurfaceReader::open(&path, width, height).expect("open");
            writer.write(&vec![7; reader.len()]);
            let mut out = vec![0; reader.len()];
            let mut time = |read: &mut dyn FnMut(&mut [u8])| {
                read(&mut out);
                let started = std::time::Instant::now();
                for _ in 0..30 {
                    read(&mut out);
                }
                started.elapsed().as_secs_f64() * 1000.0 / 30.0
            };
            let swapped = time(&mut |out| {
                assert!(reader.read_into(out));
                for pixel in out.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
            });
            let whole = time(&mut |out| assert!(reader.read_into(out)));
            let caret = time(&mut |out| {
                let caret = Region {
                    x: 400,
                    y: 300,
                    width: 2,
                    height: 40,
                };
                assert!(reader.read_region_into(out, caret));
            });
            println!(
                "{name}: whole and swapped {swapped:.3} ms, whole {whole:.3} ms, caret {caret:.4} ms"
            );
        }
        let _ = std::fs::remove_dir_all(folder);
    }
}
