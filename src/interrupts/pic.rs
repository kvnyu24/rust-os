use pic8259::ChainedPics;
use spin::Mutex;

// Standard offset for PIC1, after CPU exceptions (0-31)
pub const PIC_1_OFFSET: u8 = 32;
// PIC2 starts after PIC1's 8 interrupts
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// Ensure offsets don't overlap with CPU exceptions or each other
const_assert!(PIC_1_OFFSET >= 32);
const_assert!(PIC_2_OFFSET >= PIC_1_OFFSET + 8);
const_assert!(PIC_2_OFFSET + 8 <= 256);

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe {
    // Safety: The chosen interrupt offsets don't overlap with exceptions
    ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    /// Convert interrupt index to raw u8 value
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert interrupt index to usize, useful for array indexing
    #[inline]
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
    
    /// Check if a raw interrupt number corresponds to this PIC interrupt
    pub fn is_pic_interrupt(interrupt_id: u8) -> bool {
        interrupt_id >= PIC_1_OFFSET && interrupt_id < PIC_2_OFFSET + 8
    }

    /// Get the corresponding PIC number (1 or 2) for an interrupt
    pub fn get_pic_number(interrupt_id: u8) -> Option<u8> {
        if !Self::is_pic_interrupt(interrupt_id) {
            return None;
        }
        if interrupt_id < PIC_2_OFFSET {
            Some(1)
        } else {
            Some(2)
        }
    }
}