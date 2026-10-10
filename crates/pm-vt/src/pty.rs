//! The pseudoterminal a shell or an agent CLI runs in.
//!
//! The pty is the only part of the crate that owns threads, and it owns them
//! so that the caller never waits on the child. Reading a pty blocks, so a
//! reader thread does it and hands whole chunks over a bounded channel; the
//! caller pumps the channel whenever it is woken, and `notify` is what wakes
//! it. Writing can block as well, on a child that is not reading, so a writer
//! thread takes the bytes off a channel of its own. Killing a child and
//! waiting for it to go is done on a thread started for the purpose.

use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{
    Receiver, Sender, SyncSender, TryRecvError, TrySendError, channel, sync_channel,
};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;

use pm_host::{CommandBuilder, ExitStatus, Location, PtyChild, PtyControl};

/// How many bytes the reader thread hands over at a time.
const CHUNK: usize = 8192;

/// How many chunks may wait in the channel before the reader thread stops
/// reading, which is what leaves a child that writes faster than the window
/// draws blocked on its own pty rather than filling the editor's memory.
const QUEUED_CHUNKS: usize = 512;

/// How many bytes one [`Pty::read`] takes at most.
///
/// What is left stays queued and the caller is woken again for it, so a
/// child writing a hundred megabytes costs a frame a megabyte at a time
/// instead of one frame that never ends.
const READ_BUDGET: usize = 1024 * 1024;

/// How long the reader thread waits between looks at a child whose pty has
/// closed, for the child to be done exiting.
const REAP_PAUSE: Duration = Duration::from_millis(20);

/// How many looks it takes before it stops waiting.
///
/// A child that closes its pty and goes on running is a daemon, not one that
/// is exiting slowly, and the caller learns of it the way it always would.
const REAP_LOOKS: usize = 100;

/// Told to the caller whenever the child has written something.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// The process a pty runs, shared by the threads that look at it.
type SharedChild = Arc<Mutex<PtyChild>>;

/// A child process, the pty it runs in and the bytes it has written.
pub struct Pty {
    /// The master side, which is what a resize is applied to.
    master: PtyControl,
    /// Bytes on their way to the writer thread and from it to the child.
    input: Sender<Vec<u8>>,
    /// Chunks the reader thread has read, oldest first.
    output: Receiver<Vec<u8>>,
    /// The process itself, so it can be waited on and killed, shared with
    /// the reader thread so that it can say when the child has exited.
    child: SharedChild,
    /// Whether the caller has been woken and has not read since.
    wake: Waker,
    /// Whether the child's end of the pty has closed.
    closed: bool,
}

/// Wakes the caller once for however many chunks arrive before it reads.
#[derive(Clone)]
struct Waker {
    /// Whether a wake has been sent that the caller has not yet read after.
    pending: Arc<AtomicBool>,
    /// What wakes the caller.
    notify: Notify,
}

impl Waker {
    /// A waker over `notify` with no wake pending.
    fn new(notify: Notify) -> Self {
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            notify,
        }
    }

    /// Wakes the caller, unless a wake is already on its way.
    fn wake(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            (self.notify)();
        }
    }

    /// Wakes the caller whether or not a wake is already on its way.
    fn wake_always(&self) {
        self.pending.store(true, Ordering::Release);
        (self.notify)();
    }

    /// Says the caller is reading, so the next chunk wakes it again.
    fn clear(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

impl Pty {
    /// Starts `command` in a pty of `cols` by `rows`, rooted at `cwd`.
    pub fn spawn(
        command: CommandBuilder,
        cwd: &Location,
        cols: usize,
        rows: usize,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let pty = cwd.host.pty(command, &cwd.path, cols, rows)?;
        let child = Arc::new(Mutex::new(pty.child));
        let reaped = child.clone();
        let reader = pty.reader;
        let writer = pty.writer;
        let wake = Waker::new(notify);
        let output = spawn_reader(reader, reaped, wake.clone())?;
        let input = spawn_writer(writer)?;

        Ok(Self {
            master: pty.control,
            input,
            output,
            child,
            wake,
            closed: false,
        })
    }

    /// Takes what the child has written since the last call, up to
    /// [`READ_BUDGET`] bytes.
    ///
    /// Whatever is left past the budget stays queued, and the caller is
    /// woken again so it comes back for it on its next turn.
    pub fn read(&mut self) -> Vec<u8> {
        self.wake.clear();
        let mut bytes = Vec::new();
        while bytes.len() < READ_BUDGET {
            match self.output.try_recv() {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(TryRecvError::Empty) => return bytes,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    return bytes;
                }
            }
        }
        self.wake.wake();
        bytes
    }

    /// Hands `bytes` to the writer thread for the child's input.
    ///
    /// This returns at once: a child that is not reading holds up the writer
    /// thread, never the caller.
    pub fn write(&mut self, bytes: &[u8]) {
        if self.input.send(bytes.to_vec()).is_err() {
            self.closed = true;
        }
    }

    /// Tells the child the window is now `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let _ = self.master.resize(cols, rows);
    }

    /// Whether the child is still running.
    pub fn is_running(&mut self) -> bool {
        !self.closed && matches!(self.status(), Ok(None))
    }

    /// Ends the child, if it is still running.
    ///
    /// The kill runs on a thread of its own, since ending a child means
    /// waiting to see whether it went; the caller is woken once it has, to
    /// read how it exited.
    pub fn kill(&mut self) {
        let wake = self.wake.clone();
        end(self.child.clone(), move |_| wake.wake_always());
    }

    /// The code the child exited with, once it has exited.
    pub fn exit_code(&mut self) -> Option<u32> {
        self.status()
            .ok()
            .flatten()
            .map(|status| status.exit_code())
    }

    /// The signal that ended the child, once a signal has.
    pub fn exit_signal(&mut self) -> Option<String> {
        self.status()
            .ok()
            .flatten()
            .and_then(|status| status.signal().map(str::to_owned))
    }

    /// How the child exited, once it has.
    ///
    /// A child another thread is busy killing or reaping is taken to be
    /// running still, rather than waited for: that thread wakes the caller
    /// when it is done, and the caller asks again then.
    fn status(&self) -> std::io::Result<Option<ExitStatus>> {
        match self.child.try_lock() {
            Ok(mut child) => child.try_wait(),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Poisoned(_)) => {
                Err(std::io::Error::other("the child's lock was poisoned"))
            }
        }
    }
}

impl Drop for Pty {
    /// Kills the child, so closing a pane does not leave a shell behind.
    ///
    /// The kill and the wait for the child to be gone happen on a thread of
    /// their own, so closing a pane returns at once.
    fn drop(&mut self) {
        end(self.child.clone(), |child| {
            let _ = child.wait();
        });
    }
}

/// Starts the thread that reads the child's output into a bounded channel.
///
/// It wakes the caller through `wake` as chunks arrive, and once the pty
/// has closed it waits a moment for the child to finish exiting, then drops
/// its end of the channel and wakes the caller a last time.
fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    reaped: SharedChild,
    wake: Waker,
) -> std::io::Result<Receiver<Vec<u8>>> {
    let (sender, output) = sync_channel(QUEUED_CHUNKS);
    std::thread::Builder::new()
        .name("pm-vt-pty".to_owned())
        .spawn(move || {
            let mut buffer = [0u8; CHUNK];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        if !hand_over(&sender, buffer[..read].to_vec(), &wake) {
                            break;
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            for _ in 0..REAP_LOOKS {
                let exited = reaped
                    .lock()
                    .map_or(true, |mut child| !matches!(child.try_wait(), Ok(None)));
                if exited {
                    break;
                }
                std::thread::sleep(REAP_PAUSE);
            }
            drop(sender);
            wake.wake_always();
        })?;
    Ok(output)
}

/// Puts `chunk` on the channel and wakes the caller for it, and says
/// whether the caller is still there to read it.
///
/// A full channel wakes the caller before the thread blocks on it, so the
/// caller is never left asleep with the reader waiting for it to read.
fn hand_over(sender: &SyncSender<Vec<u8>>, chunk: Vec<u8>, wake: &Waker) -> bool {
    let sent = match sender.try_send(chunk) {
        Ok(()) => true,
        Err(TrySendError::Full(chunk)) => {
            wake.wake();
            sender.send(chunk).is_ok()
        }
        Err(TrySendError::Disconnected(_)) => false,
    };
    if sent {
        wake.wake();
    }
    sent
}

/// Starts the thread that writes to the child's input what the caller hands
/// it, and returns the channel to hand it through.
///
/// The thread ends when the pty is dropped and the channel with it, or when
/// the child's input has closed.
fn spawn_writer(mut writer: Box<dyn Write + Send>) -> std::io::Result<Sender<Vec<u8>>> {
    let (input, bytes) = channel::<Vec<u8>>();
    std::thread::Builder::new()
        .name("pm-vt-pty-input".to_owned())
        .spawn(move || {
            for chunk in bytes {
                if writer.write_all(&chunk).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        })?;
    Ok(input)
}

/// Kills `child` on a detached thread, then runs `after` on it there.
///
/// A thread that cannot be started leaves the child to the reader thread,
/// which already reaps it once its pty closes.
fn end(child: SharedChild, after: impl FnOnce(&mut PtyChild) + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("pm-vt-pty-kill".to_owned())
        .spawn(move || {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                after(&mut child);
            }
        });
}
