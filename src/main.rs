#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

// use embedded_graphics::{
//     pixelcolor::BinaryColor::On as Black,
//     prelude::*,
//     primitives::{Line, PrimitiveStyle},
// };
// use epd_waveshare::{epd1in54::*, prelude::*};

use arduino_hal::{hal::Atmega, usart::UsartOps, Usart};
use core::cell;
use panic_halt as _;

const PRESCALER: u32 = 1024;
const TIMER_COUNTS: u32 = 125;
const MILLIS_INCREMENT: u32 = PRESCALER * TIMER_COUNTS / 16000;

static MILLIS_COUNTER: avr_device::interrupt::Mutex<cell::Cell<u32>> =
    avr_device::interrupt::Mutex::new(cell::Cell::new(0));

fn millis_init(tc0: arduino_hal::pac::TC0) {
    // Configure the timer for the above interval (in CTC mode)
    // and enable its interrupt.
    tc0.tccr0a.write(|w| w.wgm0().ctc());
    tc0.ocr0a.write(|w| w.bits(TIMER_COUNTS as u8));
    tc0.tccr0b.write(|w| match PRESCALER {
        8 => w.cs0().prescale_8(),
        64 => w.cs0().prescale_64(),
        256 => w.cs0().prescale_256(),
        1024 => w.cs0().prescale_1024(),
        _ => panic!(),
    });
    tc0.timsk0.write(|w| w.ocie0a().set_bit());

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

fn millis() -> u32 {
    avr_device::interrupt::free(|cs| MILLIS_COUNTER.borrow(cs).get())
}

// ----------------

enum MenuState {
    Main,
    Lamp1,
    Lamp2,
    Lamp3,
    Lamp4,
    Lamp5,
    Lamp6,
    Lamp7,
    Lamp8,
}

struct Menu {
    mode: MenuState,
    last_update: u32,
}

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);
    let mut adc = arduino_hal::Adc::new(dp.ADC, Default::default());

    millis_init(dp.TC0);

    // replace pins with correct ones
    let dir_buttons = pins.a0.into_analog_input(&mut adc);
    let poti_left_x = pins.a1.into_analog_input(&mut adc);
    let poti_left_y = pins.a2.into_analog_input(&mut adc);
    let poti_right_x = pins.a3.into_analog_input(&mut adc);
    let poti_right_y = pins.a4.into_analog_input(&mut adc);

    let mut menu = Menu {
        mode: MenuState::Main,
        last_update: 0,
    };

    loop {
        match dir_buttons.analog_read(&mut adc) {
            // replace with values, these are switch positions
            // Lamp1: Tischlampen
            1 => match menu.mode {
                MenuState::Main => {
                    menu.mode = MenuState::Lamp1;
                    menu.last_millis = millis();
                }
                MenuState::Lamp1 => {
                    unimplemented!(); // toggle lamp1
                }
                _ => {}
            },
            2 => match menu.mode {
                Button::Main => {
                    menu.mode = Menu::Lamp2;
                    menu.last_millis = millis();
                }
                Button::Lamp2 => {
                    unimplemented!(); // toggle Lamp2
                }
            },
            3 => {}
            4 => {}
            5 => {}
            6 => {}
            7 => {}
            8 => {}
            _ => {}
        }

        match (
            poti_left_x.analog_read(&mut adc),
            poti_left_y.analog_read(&mut adc),
            poti_right_x.analog_read(&mut adc),
            poti_right_y.analog_read(&mut adc),
        ) {
            // replace with values, these are switch positions
            // left up, right down (rotating to right)
            (7, 7, 7, 7) => {
                send_data(&mut serial, 0b0000_0000_0000_0000, 0, 0, 0);
            }
            // left down, right up (rotating to left)
            (4, 1, 1, 1) => {}
            // both up
            (1...2, 1...2, 1...2, 1...2) => {}
            // both down
            (3, 3, 2, 2) => {}
            (1, 4, 2, 2) => {}
            // both left
            (5, 5, 2, 2) => {}
            // both right
            (6, 6, 2, 2) => {}
            _ => {}
        }
    }
}

/// send data to lamps
/// * `mask`: 16 bit mask
/// * `brightness`: -127 to 127 (relative)
/// * `gamma`: -127 to 127 (red < blue, relative)
/// * `position`: -127 to 127 (left < right, relative)
fn send_data<U, P1, P2>(
    serial: &mut Usart<U, P1, P2>,
    mask: u16,
    brightness: i8,
    gamma: i8,
    position: i8,
) where
    U: UsartOps<Atmega, P1, P2>,
{
    serial.write_byte(0);
    serial.write_byte(0);
    serial.write_byte(mask as u8);
    serial.write_byte((mask >> 8) as u8);
    serial.write_byte((brightness + 127) as u8);
    serial.write_byte((gamma + 127) as u8);
    serial.write_byte((position + 127) as u8);
}
