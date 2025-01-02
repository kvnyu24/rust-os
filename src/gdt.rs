use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use lazy_static::lazy_static;

// Index for the double fault IST entry
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// Size of the interrupt stack (20 KiB)
const STACK_SIZE: usize = 4096 * 5;

// Ensure IST index is valid
const_assert!(DOUBLE_FAULT_IST_INDEX < 7);

#[repr(align(16))] // Ensure stack is properly aligned
struct InterruptStack([u8; STACK_SIZE]);

static mut INTERRUPT_STACK: InterruptStack = InterruptStack([0; STACK_SIZE]);

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        
        // Set up the interrupt stack table entry
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            let stack_start = VirtAddr::from_ptr(unsafe { &INTERRUPT_STACK });
            let stack_end = stack_start + STACK_SIZE;
            // Ensure stack pointer is aligned
            stack_end.align_down(16u64)
        };

        // Initialize privilege stack table to prevent triple faults
        tss.privilege_stack_table[0] = VirtAddr::zero();
        
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        
        // Add required segments in correct order
        let kernel_data = gdt.add_entry(Descriptor::kernel_data_segment());
        let kernel_code = gdt.add_entry(Descriptor::kernel_code_segment());
        let user_data = gdt.add_entry(Descriptor::user_data_segment());
        let user_code = gdt.add_entry(Descriptor::user_code_segment());
        let tss = gdt.add_entry(Descriptor::tss_segment(&TSS));

        (gdt, Selectors { 
            kernel_code,
            kernel_data,
            user_code,
            user_data,
            tss,
        })
    };
}

struct Selectors {
    kernel_code: SegmentSelector,
    kernel_data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, DS, Segment};

    // Load GDT and segment registers
    GDT.0.load();
    unsafe {
        // Set code and data segments
        CS::set_reg(GDT.1.kernel_code);
        DS::set_reg(GDT.1.kernel_data);
        
        // Load TSS
        load_tss(GDT.1.tss);
    }
}