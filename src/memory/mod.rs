pub mod heap;

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{
        PageTable, PageTableFlags, PhysFrame, Size4KiB, FrameAllocator,
        Mapper, Page, OffsetPageTable, Translate,
    },
    VirtAddr, PhysAddr,
};
use spin::Mutex;
use alloc::vec::Vec;
use lazy_static::lazy_static;

const PAGE_SIZE: usize = 4096;
const PROGRAM_BASE: u64 = 0x400000;

lazy_static! {
    pub(crate) static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> =
        Mutex::new(None);
    pub(crate) static ref FRAME_ALLOCATOR_INITIALIZED: spin::Once<()> = spin::Once::new();
}

#[derive(Debug)]
pub struct MemorySpace {
    page_table: OffsetPageTable<'static>,
    heap_start: VirtAddr,
    heap_size: usize,
    code_start: VirtAddr,
    code_size: usize,
}

impl MemorySpace {
    pub fn new() -> Result<Self, &'static str> {
        ensure_frame_allocator_initialized()?;
        let mut guard = FRAME_ALLOCATOR.lock();
        let frame_allocator = guard.as_mut().unwrap();
        
        // Create new page table using the physical memory offset
        let page_table = unsafe {
            let level_4_table = active_level_4_table(VirtAddr::new(0xffff_8000_0000_0000));
            OffsetPageTable::new(level_4_table, VirtAddr::new(0xffff_8000_0000_0000))
        };
            
        Ok(Self {
            page_table,
            heap_start: VirtAddr::new(0x4000_0000_0000),
            heap_size: 1024 * 1024, // 1MB heap
            code_start: VirtAddr::new(0x0000_0000_0000),
            code_size: 1024 * 1024, // 1MB code segment
        })
    }

    pub fn load_program(&mut self, program: &[u8]) -> Result<(), &'static str> {
        let mut guard = FRAME_ALLOCATOR.lock();
        let frame_allocator = guard.as_mut().unwrap();
        
        let num_pages = (program.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        let mut allocated_frames = Vec::new();
        
        for i in 0..num_pages {
            let page_addr = VirtAddr::new(PROGRAM_BASE + (i * PAGE_SIZE) as u64);
            let page = Page::<Size4KiB>::containing_address(page_addr);
            let frame = frame_allocator.allocate_frame()
                .ok_or("Failed to allocate frame for program")?;
            allocated_frames.push((page, frame.clone()));
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
            
            unsafe {
                match self.page_table.map_to(page, frame, flags, frame_allocator) {
                    Ok(tlb) => tlb.flush(),
                    Err(err) => {
                        // Cleanup on error: unmap all previously mapped pages
                        for (mapped_page, _) in allocated_frames.iter() {
                            if let Ok((_frame, tlb)) = self.page_table.unmap(*mapped_page) {
                                tlb.flush();
                            }
                        }
                        return Err("Failed to map page");
                    }
                };

                let start = i * PAGE_SIZE;
                let end = core::cmp::min((i + 1) * PAGE_SIZE, program.len());
                let dest = self.page_table.translate_addr(page_addr)
                    .ok_or("Failed to translate virtual address")?
                    .as_u64() as *mut u8;
                core::ptr::copy_nonoverlapping(
                    program[start..end].as_ptr(),
                    dest,
                    end - start
                );
            }
        }
        Ok(())
    }

    pub fn entry_point(&self) -> usize {
        PROGRAM_BASE as usize
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
    total_frames: Option<usize>,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
            total_frames: None,
        }
    }
    
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    fn count_total_frames(&self) -> usize {
        self.usable_frames().count()
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.total_frames.is_none() {
            self.total_frames = Some(self.count_total_frames());
        }
        
        if self.next >= self.total_frames.unwrap() {
            return None;
        }
        
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub fn init_frame_allocator(memory_map: &'static MemoryMap) {
    FRAME_ALLOCATOR_INITIALIZED.call_once(|| {
        let mut allocator = FRAME_ALLOCATOR.lock();
        if allocator.is_some() {
            panic!("Frame allocator already initialized");
        }
        *allocator = Some(unsafe { BootInfoFrameAllocator::init(memory_map) });
    });
}

pub fn ensure_frame_allocator_initialized() -> Result<(), &'static str> {
    if FRAME_ALLOCATOR_INITIALIZED.r#try().is_some() {
        Ok(())
    } else {
        Err("Frame allocator not initialized")
    }
}

pub fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) }
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}