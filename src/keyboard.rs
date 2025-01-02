use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, KeyCode, KeyState, ScancodeSet1};
use crate::vga_buffer;
use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Context, Poll}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// A buffer size of 100 scancodes should be enough for most use cases
const QUEUE_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum KeyEvent {
    Char(char),
    SpecialKey(KeyCode),
}

pub struct KeyboardStream {
    _private: (),
}

impl KeyboardStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(QUEUE_SIZE))
            .expect("KeyboardStream::new should only be called once");
        KeyboardStream { _private: () }
    }
}

impl Stream for KeyboardStream {
    type Item = KeyEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let queue = SCANCODE_QUEUE.get().expect("scancode queue not initialized");
        
        // Fast path for empty queue
        if let Ok(scancode) = queue.pop() {
            return match process_scancode(scancode) {
                Some(event) => Poll::Ready(Some(event)),
                None => Poll::Ready(None),
            };
        }

        WAKER.register(cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                WAKER.take();
                match process_scancode(scancode) {
                    Some(event) => Poll::Ready(Some(event)),
                    None => Poll::Ready(None),
                }
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

pub(crate) fn add_scancode(scancode: u8) {
    if let Some(queue) = SCANCODE_QUEUE.get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

fn process_scancode(scancode: u8) -> Option<KeyEvent> {
    lazy_static::lazy_static! {
        static ref KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            spin::Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::Ignore));
    }

    let mut keyboard = KEYBOARD.lock();
    
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => Some(KeyEvent::Char(character)),
                DecodedKey::RawKey(key) => Some(KeyEvent::SpecialKey(key)),
            }
        } else {
            None
        }
    } else {
        None
    }
}

pub fn init() {
    KeyboardStream::new();
} 