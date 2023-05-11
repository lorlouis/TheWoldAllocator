#![feature(allocator_api)]
#![feature(atomic_from_ptr)]

use std::alloc::{GlobalAlloc, System, Layout};

use backtrace::Backtrace;

use std::cell::Cell;

use lazy_static::lazy_static;

use std::sync::{Mutex, MutexGuard, PoisonError, atomic::{AtomicUsize, Ordering}};

use lockfree::map::Map;

use std::collections::BTreeMap;

const SAMPLE_EVERY: usize = 512 * 1024;

static ALLOC_SIZE: Mutex<usize> = Mutex::new(0);

lazy_static! {
    static ref BT_TO_ATOMIC: Mutex<BTreeMap<Backtrace, AtomicUsize>> = Mutex::new(BTreeMap::default());

    static ref ALLOC_PTR_TO_ATOMIC_PTR: Map<usize, usize> = Map::default();
}

trait IgnorePoison {
    type Inner;
    fn lock_ingore_poison(&self) -> MutexGuard<Self::Inner>;
}

impl<T> IgnorePoison for Mutex<T> {
    type Inner = T;

    fn lock_ingore_poison(&self) -> MutexGuard<Self::Inner> {
        match self.lock() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("lock poisoned: {}, continuing", e);
                e.into_inner()
            }
        }
    }

}

pub struct TheWorld;

// stolen from dhat-rs
struct IgnoreAllocs {
    was_already_ignoring_allocs: bool,
}

thread_local!{
    static IGNORE_ALLOCS: Cell<bool> = Cell::new(false);
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

impl TheWorld {
    pub fn serialize_state() -> String {
        todo!()
    }
}

unsafe impl GlobalAlloc for TheWorld {

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ignore_alloc = IgnoreAllocs::new();
        let sys_alloc = System.alloc(layout);


        {
            //let cur_alloc_size = ALLOC_SIZE.lock();
            if ignore_alloc.was_already_ignoring_allocs {
                return sys_alloc
            }

            if *cur_alloc_size > SAMPLE_EVERY {
                *cur_alloc_size = 0;
                return sys_alloc;
            }
        }


        /*
        // TODO(louis) sample
        let bt = Backtrace::new();

        let atomic_ptr = {
            let mut state = BT_TO_ATOMIC.lock_ingore_poison();
            let atomic_ptr = state.entry(bt).or_default().as_ptr();
            atomic_ptr
        };
        ALLOC_PTR_TO_ATOMIC_PTR.insert(sys_alloc as usize, atomic_ptr as usize);

        let atomic = AtomicUsize::from_ptr(atomic_ptr);
        atomic.fetch_add(1, Ordering::Relaxed);
        */
        sys_alloc
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        /*
        {
            if let Some(atomic_ptr) = ALLOC_PTR_TO_ATOMIC_PTR.remove(&(ptr as usize)) {
                let atomic = AtomicUsize::from_ptr((*atomic_ptr.val()) as *mut usize);
                let _value = atomic.fetch_sub(1, Ordering::Relaxed);
                // TODO(louis) check is value is 1 and free (Backtrace, AtomicUsize)
            }
        }
        */
        System.dealloc(ptr, layout);
    }
}
