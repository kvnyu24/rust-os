pub mod heap;

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{
        PageTable, PageTableFlags, PhysFrame, Size4KiB, FrameAllocator,
        Mapper, Page, RecursivePageTable, MappedPageTable,
    },
    VirtAddr, PhysAddr,
};
use spin::Mutex;
use alloc::vec::Vec;

pub struct MemorySpace {
    page_table: RecursivePageTable<'static>,
    heap_start: VirtAddr,
    heap_size: usize,
    code_start: VirtAddr,
    code_size: usize,
}

impl MemorySpace {
    pub fn new() -> Result<Self, &'static str> {
        let mut frame_allocator = unsafe { FRAME_ALLOCATOR.lock().as_mut().unwrap() };
        
        // Allocate a new page table
        let page_table_frame = frame_allocator.allocate_frame()
            .ok_or("Failed to allocate frame for page table")?;
            
        // Get the physical address of the page table
        let phys_addr = page_table_frame.start_address();
        
        // Map it recursively
        let recursive_index = 511;
        let recursive_addr = VirtAddr::new(0xffff_ffff_ffff_f000);
        
        unsafe {
            let page_table = &mut *(phys_addr.as_u64() as *mut PageTable);
            page_table[recursive_index].set_frame(
                page_table_frame.clone(),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            );
            
            // Create recursive page table
            let page_table = RecursivePageTable::new(
                &mut *(recursive_addr.as_mut_ptr() as *mut PageTable)
            ).map_err(|_| "Failed to create recursive page table")?;
            
            Ok(Self {
                page_table,
                heap_start: VirtAddr::new(0x4000_0000_0000),
                heap_size: 1024 * 1024, // 1MB heap
                code_start: VirtAddr::new(0x0000_0000_0000),
                code_size: 1024 * 1024, // 1MB code segment
            })
        }
    }

    pub fn load_program(&self, program: &[u8]) -> Result<(), &'static str> {
        let mut frame_allocator = unsafe { FRAME_ALLOCATOR.lock().as_mut().unwrap() };
        
        // Map program pages
        let pages = (program.len() + 4095) / 4096;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        
        for i in 0..pages {
            let page = Page::<Size4KiB>::containing_address(
                self.code_start + i * 4096
            );
            
            let frame = frame_allocator.allocate_frame()
                .ok_or("Failed to allocate frame for program")?;
                
            unsafe {
                self.page_table
                    .map_to(page, frame, flags, &mut *frame_allocator)
                    .map_err(|_| "Failed to map program page")?
                    .flush();
                    
                // Copy program data
                let start = i * 4096;
                let end = core::cmp::min((i + 1) * 4096, program.len());
                let dest = (self.code_start + i * 4096).as_mut_ptr();
                dest.copy_from(program[start..end].as_ptr(), end - start);
            }
        }
        
        Ok(())
    }

    pub fn entry_point(&self) -> usize {
        self.code_start.as_u64() as usize
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
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
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

lazy_static::lazy_static! {
    static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> =
        Mutex::new(None);
}

pub fn init(physical_memory_offset: VirtAddr) -> MappedPageTable<'static, BootInfoFrameAllocator> {
    let level_4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe { MappedPageTable::new(level_4_table, physical_memory_offset) }
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