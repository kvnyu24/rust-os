use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, KeyCode, ScancodeSet1};
use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Context, Poll}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use crate::println;

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
        let queue = SCANCODE_QUEUE.get().expect("not initialized");
        
        WAKER.register(cx.waker());
        match queue.pop() {
            Some(scancode) => {
                let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    if let Some(key) = decode_key(key_event) {
                        return Poll::Ready(Some(key));
                    }
                }
                Poll::Ready(None)
            }
            None => Poll::Pending,
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

fn decode_key(key_event: pc_keyboard::KeyEvent) -> Option<KeyEvent> {
    lazy_static::lazy_static! {
        static ref KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            spin::Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::Ignore));
    }

    let mut keyboard = KEYBOARD.lock();
    if let Some(key) = keyboard.process_keyevent(key_event) {
        match key {
            DecodedKey::Unicode(character) => Some(KeyEvent::Char(character)),
            DecodedKey::RawKey(key) => Some(KeyEvent::SpecialKey(key)),
        }
    } else {
        None
    }
}

pub fn init() {
    KeyboardStream::new();
} 