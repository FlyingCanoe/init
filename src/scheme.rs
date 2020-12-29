use std::{collections::HashMap, sync::{Mutex, atomic::AtomicUsize}};

use syscall::SchemeMut;

pub struct DemonScheme {
    conection: HashMap<String, Option<usize>>,
    next_id: usize,
}

impl SchemeMut for DemonScheme {
    fn open(&mut self, path: &[u8], flags: usize, uid: u32, gid: u32) -> syscall::Result<usize> {
        match self.conection.get_mut(std::str::from_utf8(path).unwrap()) {
            Some(Some(id)) => Err(syscall::Error::new(syscall::EBUSY)),
            Some(none) => {
                let id = self.next_id;
                self.next_id += 1;

                *none = Some(id);
                Ok(id)
            },
            None => Err(syscall::Error::new(syscall::EBUSY)),
        }

        //Err(syscall::Error::new(syscall::ENOENT))
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> syscall::Result<usize> {
        Err(syscall::Error::new(syscall::EBADF))
    }

    fn write(&mut self, id: usize, buf: &[u8]) -> syscall::Result<usize> {
        Err(syscall::Error::new(syscall::EBADF))
    }

    fn close(&mut self, id: usize) -> syscall::Result<usize> {
        Err(syscall::Error::new(syscall::EBADF))
    }
}
