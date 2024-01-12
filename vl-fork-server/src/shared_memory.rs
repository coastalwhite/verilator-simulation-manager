use std::ffi::{OsString, OsStr};
use std::io;
use std::os::unix::prelude::OsStrExt;
use std::ptr;
use std::sync::{
    atomic::{self, AtomicU16},
    OnceLock,
};

#[must_use]
pub struct SharedMemory<const N: usize> {
    data: *mut u8,

    fd: libc::c_int,

    /// Null-terminated bytestring that specifies the SharedMemory uniquely
    id: OsString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

impl<const N: usize> SharedMemory<N> {
    pub fn new(access: Access) -> io::Result<Self> {
        static PROCESS_ID: OnceLock<u32> = OnceLock::new();
        static BUFFER_ID: AtomicU16 = AtomicU16::new(0);

        let process_id = PROCESS_ID.get_or_init(rand::random);
        let buffer_id = BUFFER_ID.fetch_add(1, atomic::Ordering::SeqCst);

        if buffer_id == u16::MAX {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Too many shared memory buffers allocated",
            ));
        }

        let id = format!("vfs-{process_id}-{buffer_id}");
        let id = OsString::from(id);

        let mut oflag = 0;

        oflag |= match access {
            Access::ReadOnly => libc::O_RDONLY,
            Access::ReadWrite => libc::O_RDWR,
        };

        // Create if it does not exist yet
        oflag |= libc::O_CREAT;

        // User has read and write permissions
        let mode = libc::S_IRUSR | libc::S_IWUSR;
        let mode = mode as libc::mode_t;

        let fd = unsafe { libc::shm_open(id.as_bytes().as_ptr().cast(), oflag, mode) };

        if fd < 0 {
            return Err(dbg!(std::io::Error::last_os_error()));
        }

        let result = unsafe { libc::ftruncate(fd, N as libc::off_t) };

        if result < 0 {
            return Err(dbg!(std::io::Error::last_os_error()));
        }

        let data = unsafe {
            libc::mmap(
                ptr::null_mut(),
                N as libc::size_t,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if (data as isize) == -1 {
            return Err(std::io::Error::last_os_error());
        }

        let data = data.cast();

        Ok(Self { id, fd, data })
    }

    pub fn id(&self) -> &OsStr {
        &self.id
    }

    pub fn get(&self) -> *const u8 {
        self.data
    }

    pub fn get_mut(&self) -> *mut u8 {
        self.data
    }

    pub fn clean(self) -> io::Result<()> {
        let result = unsafe { libc::shm_unlink(self.id.as_bytes().as_ptr().cast()) };

        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let result = unsafe { libc::close(self.fd) };

        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }
}
