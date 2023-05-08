#![feature(allocator_api)]
#![feature(atomic_from_ptr)]

use std::alloc::{GlobalAlloc, System, Layout};

use std::mem;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU64, Ordering};
use std::backtrace::Backtrace;
use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use std::collections::hash_map::RandomState;

lazy_static! {
    static ref ALLOC_STATE: DashMap<String, Box<AtomicU64, System>, RandomState> = DashMap::default();
}

pub struct TheWorld;

// stolen from dhat-rs
struct IgnoreAllocs {
    was_already_ignoreing_allocs: bool,
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
        /*
        if ignore_allocs.was_already_ignoreing_allocs {
            return System.alloc(layout)
        }
        */
        let rand = RNG.with(|rng| rng.u32(0..100));

        let atomic_ptr = if rand <= 3 && !ignore_allocs.was_already_ignoreing_allocs {
            let bt = Backtrace::capture().to_string();
            let atomic = ALLOC_STATE.entry(bt)
                .or_insert_with(|| Box::new_in(AtomicU64::new(0), System));
            atomic.fetch_add(1, Ordering::Relaxed);
            atomic.as_ref().as_ptr()
        }
        else {
            0 as *mut u64
        };

        let alloc_surplus = mem::size_of::<*mut u64>().max(layout.align());
        let alloc_size = layout.size() + alloc_surplus;
        let alloc_align = layout.align().max(mem::align_of::<*mut u64>());


        let new_layout = Layout::from_size_align(alloc_size, alloc_align).unwrap();

        let system_ptr = System.alloc(new_layout);
        let final_ptr = system_ptr.add(alloc_surplus);
        let ptr_ptr = final_ptr.sub(mem::size_of::<*mut u64>());
        *(ptr_ptr as *mut usize) = atomic_ptr as usize;

        println!("sys: {:x} alloc: {:x}: {} {}", system_ptr as usize, final_ptr as usize, alloc_size, alloc_align);

        final_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let ignore_allocs = IgnoreAllocs::new();
        if ignore_allocs.was_already_ignoreing_allocs {
            System.dealloc(ptr, layout);
            return
        }
        let alloc_surplus = mem::size_of::<*mut u64>().max(layout.align());
        let alloc_size = layout.size() + alloc_surplus;
        let alloc_align = layout.align().max(mem::align_of::<*mut u64>());

        let ptr_ptr = ptr.sub(mem::size_of::<*mut u64>()) as *mut u64;
        if !ptr_ptr.is_null() {
            let atomic = AtomicU64::from_ptr(ptr_ptr);
            atomic.fetch_sub(1, Ordering::AcqRel);
        }

        let new_layout = Layout::from_size_align(alloc_size, alloc_align).unwrap();
        println!("arg: {:x} free: {:x}: {} {}", ptr as usize, ptr.sub(alloc_surplus) as usize, alloc_size, alloc_align);
        System.dealloc(ptr.sub(alloc_surplus), new_layout)
    }
}
