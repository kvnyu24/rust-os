use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::registers::segmentation::{CS, DS, SS, Segment};
use x86_64::PrivilegeLevel;
use lazy_static::lazy_static;
use static_assertions::const_assert;

// Constants for stack sizes and IST indices
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const PAGE_FAULT_IST_INDEX: u16 = 1;
pub const GENERAL_PROTECTION_IST_INDEX: u16 = 2;

const INTERRUPT_STACK_SIZE: usize = 4096 * 5;  // 20 KiB
const PRIVILEGE_STACK_SIZE: usize = 4096 * 3;  // 12 KiB per privilege level

// Validate IST indices
const_assert!(DOUBLE_FAULT_IST_INDEX < 7);
const_assert!(PAGE_FAULT_IST_INDEX < 7);
const_assert!(GENERAL_PROTECTION_IST_INDEX < 7);

#[derive(Debug)]
pub enum GDTError {
    InvalidSelector,
    InvalidPrivilegeLevel,
    StackNotAligned,
}

#[repr(align(16))]
struct InterruptStack([u8; INTERRUPT_STACK_SIZE]);

#[repr(align(16))]
struct PrivilegeStack([u8; PRIVILEGE_STACK_SIZE]);

// Stacks for different types of interrupts
static mut DOUBLE_FAULT_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);
static mut PAGE_FAULT_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);
static mut GP_FAULT_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);

// Stacks for different privilege levels
static mut PRIVILEGE_LEVEL_STACKS: [PrivilegeStack; 3] = [
    PrivilegeStack([0; PRIVILEGE_STACK_SIZE]),
    PrivilegeStack([0; PRIVILEGE_STACK_SIZE]),
    PrivilegeStack([0; PRIVILEGE_STACK_SIZE]),
];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        
        // Set up interrupt stack table entries
        unsafe {
            tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
                let stack_start = VirtAddr::from_ptr(&DOUBLE_FAULT_STACK);
                stack_start + INTERRUPT_STACK_SIZE
            };
            
            tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
                let stack_start = VirtAddr::from_ptr(&PAGE_FAULT_STACK);
                stack_start + INTERRUPT_STACK_SIZE
            };
            
            tss.interrupt_stack_table[GENERAL_PROTECTION_IST_INDEX as usize] = {
                let stack_start = VirtAddr::from_ptr(&GP_FAULT_STACK);
                stack_start + INTERRUPT_STACK_SIZE
            };

            // Initialize privilege stack table
            for (i, stack) in PRIVILEGE_LEVEL_STACKS.iter().enumerate() {
                tss.privilege_stack_table[i] = VirtAddr::from_ptr(stack) + PRIVILEGE_STACK_SIZE;
            }
        }
        
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        
        // Add segments in correct order with proper access rights
        let kernel_data = gdt.add_entry(Descriptor::kernel_data_segment());
        let kernel_code = gdt.add_entry(Descriptor::kernel_code_segment());
        let user_data = gdt.add_entry(Descriptor::user_data_segment());
        let user_code = gdt.add_entry(Descriptor::user_code_segment());
        
        // Add system call segment (ring 3 to ring 0 fast transitions)
        let syscall_code = gdt.add_entry(Descriptor::UserSegment(0xc0_9a_00_00_00_00_00_00));
        
        let tss = gdt.add_entry(Descriptor::tss_segment(&TSS));

        (gdt, Selectors { 
            kernel_code,
            kernel_data,
            user_code,
            user_data,
            syscall_code,
            tss,
        })
    };
}

#[derive(Debug)]
pub struct Selectors {
    kernel_code: SegmentSelector,
    kernel_data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    syscall_code: SegmentSelector,
    tss: SegmentSelector,
}

impl Selectors {
    pub fn get_selector(&self, privilege_level: PrivilegeLevel) -> Result<(SegmentSelector, SegmentSelector), GDTError> {
        match privilege_level {
            PrivilegeLevel::Ring0 => Ok((self.kernel_code, self.kernel_data)),
            PrivilegeLevel::Ring3 => Ok((self.user_code, self.user_data)),
            _ => Err(GDTError::InvalidPrivilegeLevel),
        }
    }

    pub fn get_syscall_selector(&self) -> SegmentSelector {
        self.syscall_code
    }
}

pub fn init() {
    use x86_64::instructions::tables::load_tss;

    // Load GDT and segment registers
    GDT.0.load();
    unsafe {
        // Set code and data segments for kernel mode
        CS::set_reg(GDT.1.kernel_code);
        DS::set_reg(GDT.1.kernel_data);
        SS::set_reg(GDT.1.kernel_data);
        
        // Load TSS
        load_tss(GDT.1.tss);
    }
}

pub fn get_current_privilege_level() -> PrivilegeLevel {
    let selector = CS::get_reg();
    selector.rpl()
}

pub fn switch_to_user_mode() -> Result<(), GDTError> {
    unsafe {
        let (user_code, user_data) = GDT.1.get_selector(PrivilegeLevel::Ring3)?;
        
        // Switch to user mode segments
        CS::set_reg(user_code);
        SS::set_reg(user_data);
        DS::set_reg(user_data);
    }
    
    Ok(())
}