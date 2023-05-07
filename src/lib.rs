#![feature(allocator_api)]

use std::alloc::{GlobalAlloc, System, Layout};
use std::panic::Location;
use std::io::Write;

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::RwLock;
use std::cell::Cell;

use hashbrown::HashMap;

use nix::unistd;

static mut ALLOC_STATE: Option<HashMap<&'static Location<'static>, Box<AtomicU64, System>, System>> = None;

pub struct TheWorld;

// stolen from dhat-rs
struct IgnoreAllocs {
    was_already_ignoreing_allocs: bool,
}

thread_local!{static IGNORE_ALLOCS: Cell<bool> = Cell::new(false)}

impl IgnoreAllocs {
    fn new() -> Self {
        Self {
            was_already_ignoreing_allocs: IGNORE_ALLOCS.with(|b| b.replace(true)),
        }
    }
}

impl std::ops::Drop for IgnoreAllocs {
    fn drop(&mut self) {
        if !self.was_already_ignoreing_allocs {
            IGNORE_ALLOCS.with(|b| b.set(false))
        }
    }
}
// end of the stolen code

unsafe impl GlobalAlloc for TheWorld {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ignore_allocs = IgnoreAllocs::new();
        // speed up our own allocations
        if ignore_allocs.was_already_ignoreing_allocs {
            return System.alloc(layout)
        }
        let rand = fastrand::u32(0..10000);

        if rand <= 3 {
            let mut buffer = Vec::<u8, System>::new_in(System);
            writeln!(&mut buffer, "{}", Location::caller()).unwrap();
            unistd::write(1, &buffer).unwrap();
        }
        System.alloc(layout)
    }

    #[track_caller]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}
