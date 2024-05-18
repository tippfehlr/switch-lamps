use core::cell;
use core::time::Duration;

pub const PRESCALER: u32 = 64;
pub const TIMER_COUNTS: u32 = 250;
pub const MILLIS_INCREMENT: u32 = PRESCALER * TIMER_COUNTS / 16000;

pub static MILLIS_COUNTER: avr_device::interrupt::Mutex<cell::Cell<u32>> =
    avr_device::interrupt::Mutex::new(cell::Cell::new(0));

pub fn millis_init(tc0: arduino_hal::pac::TC0) {
    // Configure the timer for the above interval (in CTC mode)
    // and enable its interrupt.
    tc0.tccr0a.write(|w| w.wgm0().ctc()); // timer control register 0a
    tc0.ocr0a.write(|w| w.bits(TIMER_COUNTS as u8)); // output compare register 0a
    tc0.tccr0b.write(|w| match PRESCALER {
        8 => w.cs0().prescale_8(),
        64 => w.cs0().prescale_64(),
        256 => w.cs0().prescale_256(),
        1024 => w.cs0().prescale_1024(),
        _ => panic!(),
    }); // timer control register 0b
    tc0.timsk0.write(|w| w.ocie0a().set_bit()); // timer interrupt mask register 0

    // Reset the global millisecond counter
    avr_device::interrupt::free(|cs| {
        MILLIS_COUNTER.borrow(cs).set(0);
    });
}

#[allow(non_snake_case)]
#[avr_device::interrupt(atmega328p)]
fn TIMER0_COMPA() {
    avr_device::interrupt::free(|cs| {
        let counter_cell = MILLIS_COUNTER.borrow(cs);
        let counter = counter_cell.get();
        counter_cell.set(counter + MILLIS_INCREMENT);
    })
}

pub fn millis() -> Duration {
    avr_device::interrupt::free(|cs| Duration::from_millis(MILLIS_COUNTER.borrow(cs).get().into()))
}
