use std::{mem, ptr};
use std::ops::{Index, IndexMut};

use crate::Result;

#[cfg(target_os = "windows")]
mod api {
    extern "C" {
        fn VirtualProtect(addr: *mut u8, len: usize, prot: i32, old_prot: *mut i32) -> i32;
    }

    pub(super) unsafe fn alloc_memory(len: usize) -> Result<*mut u8> {
        use std::alloc::{self, Layout};

        unsafe {
            let layout = Layout::from_size_align(len, api::page_size())?;
            let mem = unsafe { alloc::alloc(layout) };
            unsafe { api::set_read_write_exec(mem, len) }?;
            Ok(mem)
        }
    }

    unsafe fn set_read_write_exec(addr: *mut u8, len: usize) -> Result<()> {
        let mut old_prot = 0;
        let execute_read_write = 0x40;
        if VirtualProtect(addr, len, execute_read_write, &mut old_prot as *mut i32) != 0 {
            Ok(())
        } else {
            Err(crate::JitVmError::MapError(std::io::Error::last_os_error()))
        }
    }

    pub(super) fn page_size() -> usize {
        // TODO: use getpagesize
        4096
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod api {
    use std::os::raw::c_void;

    use crate::Result;

    pub unsafe fn alloc_memory(len: usize) -> Result<*mut c_void> {
        unsafe {
            let page_size = page_size();
            let pages = (len + page_size - 1) / page_size;
            let mem = libc::mmap(std::ptr::null_mut(), page_size * pages, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0);
            if mem == libc::MAP_FAILED {
                Err(crate::JitVmError::MapError(std::io::Error::last_os_error()))
            } else {
                Ok(mem)
            }
        }
    }

    // unsafe fn set_read_write_exec(addr: *mut u8, len: usize) -> Result<()> {
    //     unsafe {
    //         use libc;
    //         let prot = libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC;
    //         if libc::mprotect(addr as *mut libc::c_void, len, prot) == 0 {
    //             Ok(())
    //         } else {
    //             Err(crate::JitVmError::MapError(std::io::Error::last_os_error()))
    //         }
    //     }
    // }

    pub(super) fn page_size() -> usize {
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
    }
}

#[cfg(target_os = "macos")]
mod api {
    use std::os::raw::c_void;

    use crate::Result;

    pub(super) unsafe fn alloc_memory(len: usize) -> Result<*mut c_void> {
        unsafe {
            use libc;
            let page_size = page_size();
            let pages = len.div_ceil(page_size);

            let mem = libc::mmap(
                std::ptr::null_mut(),
                page_size * pages,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_JIT,
                -1,
                0,
            );
            if mem == libc::MAP_FAILED {
                Err(crate::JitVmError::MapError(std::io::Error::last_os_error()))
            } else {
                Ok(mem)
            }
        }
    }

    pub(super) fn page_size() -> usize {
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
    }
}


pub struct JitMemory {
    mem: *mut u8,
    len: usize,
}

impl JitMemory {
    pub fn alloc(len: usize) -> Result<Self> {
        let mem = unsafe { api::alloc_memory(len)? };

        Ok(JitMemory { mem: mem as *mut u8, len })
    }

    pub fn write_at(&mut self, offset: &mut usize, data: &[u8]) {
        assert!(*offset + data.len() <= self.len());

        for i in 0..data.len() {
            unsafe {
                self.mem.offset((*offset + i) as isize).write(data[i]);
            }
        }

        *offset += data.len();
    }

    // write value into mem by offset
    pub fn patch_addr_u64(&mut self, offset: usize, value: u64) {
        assert!(offset + 8 <= self.len());
        // unsafe { *(self.mem.add(offset) as *mut u64) = value }
        unsafe { ptr::write_unaligned(self.mem.add(offset) as *mut u64, value) }
    }

    // write value into mem by offset
    pub fn patch_addr_i32(&mut self, offset: usize, value: i32) {
        assert!(offset + 4 <= self.len());
        // unsafe { *(self.mem.add(offset) as *mut i32) = value }
        unsafe { ptr::write_unaligned(self.mem.add(offset) as *mut i32, value) }
    }

    pub fn as_function(&self) -> fn() {
        unsafe { mem::transmute(self.mem) }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Index<usize> for JitMemory {
    type Output = u8;

    fn index(&self, _index: usize) -> &u8 {
        unsafe { mem::transmute(self.mem.add(_index)) }
    }
}

impl IndexMut<usize> for JitMemory {
    fn index_mut(&mut self, _index: usize) -> &mut u8 {
        unsafe { mem::transmute(self.mem.add(_index)) }
    }
}
