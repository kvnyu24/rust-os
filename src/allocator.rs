use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};
use linked_list_allocator::LockedHeap;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
pub const HEAP_MAX_SIZE: usize = 1024 * 1024; // 1 MiB

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Statistics for memory usage
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static PEAK_MEMORY_USAGE: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct HeapStats {
    pub allocated_bytes: usize,
    pub allocation_count: usize,
    pub peak_memory_usage: usize,
    pub current_heap_size: usize,
}

pub fn get_heap_stats() -> HeapStats {
    HeapStats {
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        allocation_count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        peak_memory_usage: PEAK_MEMORY_USAGE.load(Ordering::Relaxed),
        current_heap_size: HEAP_SIZE,
    }
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // Ensure heap start is properly aligned
    assert!(VirtAddr::new(HEAP_START as u64).is_aligned(Page::<Size4KiB>::SIZE));
    
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Map all pages in the range
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // Initialize the allocator
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

pub fn try_expand_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    additional_size: usize,
) -> Result<(), MapToError<Size4KiB>> {
    let current_size = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let new_size = current_size + additional_size;
    
    if new_size > HEAP_MAX_SIZE {
        return Err(MapToError::FrameAllocationFailed);
    }

    let start_page = Page::containing_address(VirtAddr::new((HEAP_START + current_size) as u64));
    let end_page = Page::containing_address(VirtAddr::new((HEAP_START + new_size - 1) as u64));

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().extend(additional_size);
    }

    Ok(())
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!(
        "Allocation error: {:?}\nCurrent heap stats: {:?}",
        layout,
        get_heap_stats()
    )
}

// Track allocations
#[doc(hidden)]
pub fn track_allocation(size: usize) {
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    let current = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let mut peak = PEAK_MEMORY_USAGE.load(Ordering::Relaxed);
    while current > peak {
        match PEAK_MEMORY_USAGE.compare_exchange_weak(
            peak,
            current,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(new_peak) => peak = new_peak,
        }
    }
}

// Track deallocations
#[doc(hidden)]
pub fn track_deallocation(size: usize) {
    ALLOCATED_BYTES.fetch_sub(size, Ordering::Relaxed);
} 