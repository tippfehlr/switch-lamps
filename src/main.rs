#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]
#![allow(unused_imports)]

use embedded_graphics::{
    pixelcolor::BinaryColor::{self},
    prelude::*,
};
use epd_waveshare::{
    epd1in54::{Epd1in54, HEIGHT, WIDTH},
    prelude::*,
};

use arduino_hal::{
    clock::MHz16,
    hal::{delay::Delay, Atmega},
    port::{
        mode::{Input, Output, PullUp},
        Pin,
    },
    usart::UsartOps,
    Spi, Usart,
};
use core::{cell, marker::PhantomData, time::Duration};
use panic_halt as _;

const BUTTON_HOLD_INTERVAL: Duration = Duration::from_millis(200);
const MENU_TIMEOUT: Duration = Duration::from_secs(5);

// --------- MILLIS ----------

const PRESCALER: u32 = 64;
const TIMER_COUNTS: u32 = 250;
const MILLIS_INCREMENT: u32 = PRESCALER * TIMER_COUNTS / 16000;

static MILLIS_COUNTER: avr_device::interrupt::Mutex<cell::Cell<u32>> =
    avr_device::interrupt::Mutex::new(cell::Cell::new(0));

fn millis_init(tc0: arduino_hal::pac::TC0) {
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

fn millis() -> Duration {
    avr_device::interrupt::free(|cs| Duration::from_millis(MILLIS_COUNTER.borrow(cs).get().into()))
}

// ----------------

#[derive(Clone, PartialEq, Eq)]
enum MenuState {
    Main,
    /// wall lamps
    /// id’s 0-3
    Lamp1,
    /// desk lamps
    /// id’s 4-6
    Lamp2,
    // /// power outlets
    // /// id’s 4-5
    // Lamp3,
    // /// strahler
    // /// id 9
    // Lamp4,
    // /// Bastelecke
    // /// id 10
    // Lamp5,
    // ///
    // Lamp6,
    // Lamp7,
    // Lamp8,
    // …
}

#[derive(Clone, PartialEq, Eq)]
enum Button {
    None,

    /// increase blue
    SlideUp,

    /// decrease blue
    SlideDown,

    /// move focus point left
    SlideLeft,

    /// move focus point right
    SlideRight,

    /// if selected turn wall lamps on/off
    /// else select wall lamps
    PressTop,

    /// if selected turn desk lamps on/off
    /// else select desk lamps
    PressBottom,

    /// select next lamp
    PressRight,

    /// select previous lamp
    PressLeft,

    /// increase brightness
    RotateRight,

    /// decrease brightness
    RotateLeft,
}

// struct DisplayBuffer<
//     E: WaveshareDisplay<
//         Spi,
//         Pin<Output>,
//         Pin<Input>,
//         Pin<Output>,
//         Pin<Output>,
//         Delay<MHz16>,
//         DisplayColor = Color,
//     >,
// > {
//     epd: E,
// }

// impl<
//         E: WaveshareDisplay<
//             Spi,
//             Pin<Output>,
//             Pin<Input>,
//             Pin<Output>,
//             Pin<Output>,
//             Delay<MHz16>,
//             DisplayColor = Color,
//         >,
//     > DisplayBuffer<E>
// {
//     fn new(
//         spi: &mut Spi,
//         cs: Pin<Output>,
//         busy: Pin<Input>,
//         dc: Pin<Output>,
//         rst: Pin<Output>,
//         delay: &mut Delay<MHz16>,
//     ) -> Self {
//         let epd = Epd1in54::new(spi, cs, busy, dc, rst, delay).unwrap();
//         Self { epd }
//     }
// }

// impl<P1, P2, P3, P4, D> OriginDimensions for DisplayBuffer<P1, P2, P3, P4, D> {
//     fn size(&self) -> Size {
//         Size {
//             width: WIDTH,
//             height: HEIGHT,
//         }
//     }
// }

// impl<P1, P2, P3, P4, D> DrawTarget for DisplayBuffer<P1, P2, P3, P4, D> {
//     type Color = BinaryColor;
//     type Error = ();
//     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Pixel<Self::Color>>,
//     {
//         for pixel in pixels {}
//         Ok(())
//     }
// }

// #[link_section = ".rodata"]
// static LAMP1: [u8; 200 * 200 / 8] = [];

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);
    let mut adc = arduino_hal::Adc::new(dp.ADC, Default::default());

    millis_init(dp.TC0);

    unsafe { avr_device::interrupt::enable() };

    //TODO: replace pins with correct ones
    let dir_buttons = pins.a0.into_analog_input(&mut adc);
    let poti_left_x = pins.a1.into_analog_input(&mut adc);
    let poti_left_y = pins.a2.into_analog_input(&mut adc);
    let poti_right_x = pins.a3.into_analog_input(&mut adc);
    let poti_right_y = pins.a4.into_analog_input(&mut adc);

    // let (mut spi, _) = arduino_hal::Spi::new(
    //     dp.SPI,
    //     pins.d13.into_output(),
    //     pins.d11.into_output(),
    //     pins.d12.into_pull_up_input(),
    //     pins.d10.into_output(),
    //     arduino_hal::spi::Settings::default(),
    // );
    // let cs_pin = pins.d5.into_output();
    // let busy_in = pins.d6.into_pull_up_input();
    // let dc = pins.d7.into_output();
    // let rst = pins.d8.into_output();

    // let mut epd = Epd1in54::new(
    //     &mut spi,
    //     cs_pin,
    //     busy_in,
    //     dc,
    //     rst,
    //     &mut arduino_hal::Delay::new(),
    // )
    // .unwrap();

    // let display = DisplayBuffer { epd };

    // let style = PrimitiveStyleBuilder::new()
    //     .stroke_color(Black)
    //     .stroke_width(1)
    //     .build();

    // Line::new(Point::new(0, 120), Point::new(1, 295))
    //     .into_styled(style)
    //     .draw(&mut display);

    // epd.update_partial_frame(&mut spi, &LAMP1, 0, 0, 20, 20);

    let mut button: Button;
    let mut last_button: Button = Button::None;
    let mut last_button_hold_time: Duration = Duration::ZERO;

    let mut menu_state: MenuState = MenuState::Main;
    let mut menu_state_timeout: Duration = Duration::ZERO;

    loop {
        button = Button::None;

        if millis() - menu_state_timeout >= MENU_TIMEOUT {
            menu_state = MenuState::Main;
            update_display(&menu_state);
        }

        match dir_buttons.analog_read(&mut adc) {
            //TODO: replace with values, these are switch positions
            1 => button = Button::PressTop,
            2 => button = Button::PressLeft,
            3 => button = Button::PressRight,
            4 => button = Button::PressBottom,
            _ => {}
        }

        // sliding overwrites pressing
        match (
            poti_left_x.analog_read(&mut adc),
            poti_left_y.analog_read(&mut adc),
            poti_right_x.analog_read(&mut adc),
            poti_right_y.analog_read(&mut adc),
        ) {
            //TODO: replace with values, these are switch positions
            // left up, right down (rotating to right)
            (7, 7, 7, 7) => button = Button::RotateRight,

            // left down, right up (rotating to left)
            (4, 1, 1, 1) => button = Button::RotateLeft,

            // both up
            (1...2, 1...2, 1...2, 1...2) => button = Button::SlideUp,

            // both down
            (3, 3, 2, 2) => button = Button::SlideDown,

            // both left
            (5, 5, 2, 2) => button = Button::SlideLeft,

            // both right
            (6, 6, 2, 2) => button = Button::SlideRight,

            _ => {}
        };

        if button != Button::None
            && (button != last_button || millis() - last_button_hold_time >= BUTTON_HOLD_INTERVAL)
        {
            last_button = button.clone();
            last_button_hold_time = millis();

            match button {
                Button::RotateRight => send_data(&mut serial, get_mask(&menu_state), 10, 0, 0),
                Button::RotateLeft => send_data(&mut serial, get_mask(&menu_state), -10, 0, 0),
                Button::SlideUp => send_data(&mut serial, get_mask(&menu_state), 0, 10, 0),
                Button::SlideDown => send_data(&mut serial, get_mask(&menu_state), 0, -10, 0),
                Button::SlideLeft => send_data(&mut serial, get_mask(&menu_state), 0, 0, -10),
                Button::SlideRight => send_data(&mut serial, get_mask(&menu_state), 0, 0, 10),
                Button::PressTop => match &menu_state {
                    MenuState::Lamp1 => send_data(&mut serial, get_mask(&menu_state), -127, 0, 0),
                    _ => {
                        menu_state = MenuState::Lamp1;
                        menu_state_timeout = millis();

                        update_display(&menu_state);
                    }
                },
                Button::PressBottom => match menu_state {
                    MenuState::Lamp2 => send_data(&mut serial, get_mask(&menu_state), -127, 0, 0),
                    _ => {
                        menu_state = MenuState::Lamp2;
                        menu_state_timeout = millis();

                        update_display(&menu_state);
                    }
                },
                Button::PressLeft => {
                    increment_menu_state(&mut menu_state);

                    update_display(&menu_state);
                }
                Button::PressRight => {
                    decrement_menu_state(&mut menu_state);

                    update_display(&menu_state);
                }
                Button::None => unreachable!(),
            }
        }
    }
}

fn update_display(menu_state: &MenuState) {
    todo!();
}

fn increment_menu_state(menu_state: &mut MenuState) {
    match menu_state {
        MenuState::Main => *menu_state = MenuState::Lamp1,
        MenuState::Lamp1 => *menu_state = MenuState::Lamp2,
        MenuState::Lamp2 => *menu_state = MenuState::Main,
    }
}

fn decrement_menu_state(menu_state: &mut MenuState) {
    match menu_state {
        MenuState::Main => *menu_state = MenuState::Lamp2,
        MenuState::Lamp2 => *menu_state = MenuState::Lamp1,
        MenuState::Lamp1 => *menu_state = MenuState::Main,
    }
}

fn get_mask(menu_state: &MenuState) -> u16 {
    match menu_state {
        MenuState::Main => 0b1111111111111111,
        MenuState::Lamp1 => 0b0000000000001111,
        MenuState::Lamp2 => 0b0000000001110000,
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
