use crate::ulib::*;
use crate::usys::*;
use api::file::*;

pub struct File {
    pub fd: u32,
}
impl File {
    pub fn open(path: &str, omode: FileOpenMode) -> Result<Self, SyscallError> {
        match open(path, omode) {
            Err(v) => Err(v),
            Ok(fd) => Ok(Self { fd: fd }),
        }
    }
    pub fn from_fd(fd: u32) -> Self {
        Self { fd: fd }
    }
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        read(self.fd, buf)
    }
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        write(self.fd, buf)
    }
    pub fn state(&self) -> Result<FileState, SyscallError> {
        fstat(self.fd)
    }
    pub fn try_clone(&self) -> Result<Self, SyscallError> {
        let fd = dup(self.fd)?;
        Ok(Self { fd: fd })
    }
}
impl Drop for File {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}
