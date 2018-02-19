use std::ops::{Index, IndexMut};
use std::mem;
use std::heap::{Alloc, Layout, Heap};

#[cfg(target_os = "windows")]
extern "C" {
    fn VirtualProtect(addr: *mut u8, len: usize, prot: i32, old_prot: *mut i32) -> i32;
}

#[cfg(target_os = "windows")]
unsafe fn set_read_write_exec(addr: *mut u8, len: usize) -> bool {
    let mut old_prot = 0;
    let execute_read_write = 0x40;
    VirtualProtect(addr, len, execute_read_write, &mut old_prot as *mut i32) != 0
}

#[cfg(not(target_os = "windows"))]
unsafe fn set_read_write_exec(addr: *mut u8, len: usize) -> bool {
    use libc;
    let prot = libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC;
    libc::mprotect(addr as *mut libc::c_void, len, prot) == 0
}

pub struct JitMemory {
    mem: *mut u8,
    len: usize
}

impl JitMemory {
    pub fn alloc(len: usize) -> Option<Self> {
        let layout = Layout::from_size_align(len, 4096).unwrap(); // TODO: use getpagesize
        let mem = unsafe { Alloc::alloc(&mut Heap::default(), layout).unwrap() };
        let res = unsafe { set_read_write_exec(mem, len) };

        if res {
            Some(JitMemory { mem, len })
        } else {
            None
        }
    }

    pub fn write_at(&mut self, offset: &mut usize, data: &[u8]) {
        assert!(*offset + data.len() <= self.len());

        for i in 0..data.len() {
            unsafe { self.mem.offset((*offset + i) as isize).write(data[i]); }
        }

        *offset += data.len();
    }

    // write value into mem by offset
    pub fn patch_addr_u64(&mut self, offset: usize, value: u64) {
        assert!(offset + 8 <= self.len());
        unsafe {
            *(self.mem.add(offset) as *mut u64) = value
        }
    }

    // write value into mem by offset
    pub fn patch_addr_i32(&mut self, offset: usize, value: i32) {
        assert!(offset + 4 <= self.len());
        unsafe {
            *(self.mem.add(offset) as *mut i32) = value
        }
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
