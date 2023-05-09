#![feature(allocator_api)]
#![feature(atomic_from_ptr)]

use std::alloc::{GlobalAlloc, System, Layout};

use std::mem;
use std::ptr;
use lazy_static::lazy_static;
use std::backtrace::Backtrace;
use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};
use nix::unistd;
use std::sync::Mutex;
use std::io::Write;

use hashbrown::{HashMap, hash_map::DefaultHashBuilder};


lazy_static! {
    static ref ALLOC_STATE: Mutex<HashMap<Vec<u8, System>, Box<AtomicU64, System>, DefaultHashBuilder, System>> = Mutex::new(HashMap::new_in(System));
}

pub struct TheWorld;

// stolen from dhat-rs
struct IgnoreAllocs {
    was_already_ignoring_allocs: bool,
}

thread_local!{
    static IGNORE_ALLOCS: Cell<bool> = Cell::new(false);
    static RNG: fastrand::Rng = fastrand::Rng::with_seed(
        SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_nanos() as u64)
        .unwrap_or(4)
    );
}

impl IgnoreAllocs {
    fn new() -> Self {
        Self {
            was_already_ignoring_allocs: IGNORE_ALLOCS.with(|b| b.replace(true)),
        }
    }
}

impl std::ops::Drop for IgnoreAllocs {
    fn drop(&mut self) {
        if !self.was_already_ignoring_allocs {
            IGNORE_ALLOCS.with(|b| b.set(false))
        }
    }
}
// end of the stolen code

unsafe impl GlobalAlloc for TheWorld {

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ignore_allocs = IgnoreAllocs::new();

        let alloc_surplus = layout.align().max(mem::size_of::<*const usize>());
        let layout = Layout::from_size_align_unchecked(
            layout.size() + alloc_surplus,
            layout.align().max(mem::align_of::<*const u64>()),
        );

        let alloc = System.alloc(layout);
        if ignore_allocs.was_already_ignoring_allocs {
            alloc.write_bytes(0, mem::size_of::<*const u64>());
        }
        else {
            let rng = RNG.with(|rng| rng.i32(0..100));

            let atomic_ptr = if rng <= 3 {
                let mut buffer = Vec::new_in(System);
                write!(&mut buffer, "{}", Backtrace::capture().to_string()).unwrap();
                let ptr = ALLOC_STATE.lock().unwrap().entry(buffer)
                    .or_insert_with(|| Box::new_in(AtomicU64::new(0), System)).as_ptr();

                let counter = AtomicU64::from_ptr(ptr);
                counter.fetch_add(1, Ordering::Relaxed);

                let mut buffer = Vec::new_in(System);
                writeln!(&mut buffer, "alloc: {:x}, ptr: {:x}\n", ptr as usize, alloc.add(alloc_surplus) as usize);
                unistd::write(2, &buffer);

                ptr
            } else {
                ptr::null_mut()
            };

            ptr::write(alloc as *mut usize, atomic_ptr as usize);
        }

        alloc.add(alloc_surplus)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let alloc_surplus = layout.align().max(mem::size_of::<*const ()>());
        let layout = Layout::from_size_align_unchecked(
            layout.size() + alloc_surplus,
            layout.align().max(mem::align_of::<*const ()>()),
        );

        let atomic_ptr: *mut u64 = ptr::read(ptr as *const *mut u64);

        if !atomic_ptr.is_null() {
            let counter = AtomicU64::from_ptr(atomic_ptr);
            let mut buffer = Vec::new_in(System);
            writeln!(&mut buffer, "free: {:x}, ptr: {:x}\n", atomic_ptr as usize, ptr as usize);
            unistd::write(2, &buffer);
            counter.fetch_sub(1, Ordering::Relaxed);
        }

        System.dealloc(ptr.sub(alloc_surplus), layout);
    }
}
