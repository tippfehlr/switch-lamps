#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

use embedded_graphics::{
    mono_font::{iso_8859_9::FONT_9X15_BOLD, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use epd_waveshare::{epd1in54::Epd1in54, prelude::*};

use arduino_hal::{
    adc,
    clock::MHz16,
    delay_ms,
    hal::{delay::Delay, Atmega, Spi},
    port::{
        mode::{Input, Output, PullUp},
        Pin,
    },
    usart::UsartOps,
    Usart,
};
use core::{cell, time::Duration};
use panic_halt as _;
use ufmt::{derive::uDebug, uDisplay, uWrite, uwriteln};

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

#[derive(Clone, PartialEq, Eq, uDebug)]
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

type Epd = Epd1in54<Spi, Pin<Output>, Pin<Input<PullUp>>, Pin<Output>, Pin<Output>, Delay<MHz16>>;
enum DisplayError {}

struct Display {
    epd: Epd,
    spi: Spi,
    delay: Delay<MHz16>,
}

impl Display {
    fn new(epd: Epd, spi: Spi) -> Self {
        let delay = Delay::<arduino_hal::clock::MHz16>::new();
        Self { epd, spi, delay }
    }
    fn display_frame(&mut self) {
        self.epd
            .display_frame(&mut self.spi, &mut self.delay)
            .unwrap();
    }
}

impl OriginDimensions for Display {
    fn size(&self) -> Size {
        Size {
            width: 200,
            height: 200,
        }
    }
}

fn to_color(color: BinaryColor) -> u8 {
    color.is_on() as u8 * 255
}

impl DrawTarget for Display {
    type Color = BinaryColor;
    type Error = DisplayError;
    fn draw_iter<I>(&mut self, draw: I) -> Result<(), DisplayError>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for pixel in draw {
            self.epd
                .update_partial_frame(
                    &mut self.spi,
                    &[to_color(pixel.1)],
                    pixel.0.x.try_into().unwrap(),
                    pixel.0.y.try_into().unwrap(),
                    1,
                    1,
                )
                .unwrap();
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.epd.set_background_color(to_color(color).into());
        self.epd
            .clear_frame(&mut self.spi, &mut self.delay)
            .unwrap();
        Ok(())
    }
}

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

    let _poti_power = pins.d12.into_output_high();

    let dir_buttons = pins.a1.into_analog_input(&mut adc);
    let poti_left_x = pins.a0.into_analog_input(&mut adc);
    let poti_left_y = pins.a2.into_analog_input(&mut adc);
    let poti_right_x = pins.a3.into_analog_input(&mut adc);

    // let (mut spi, _) = arduino_hal::Spi::new(
    //     dp.SPI,
    //     pins.d13.into_output(),
    //     pins.d11.into_output(),
    //     pins.d12.into_pull_up_input(),
    //     pins.d10.into_output(),
    //     arduino_hal::spi::Settings::default(),
    // );
    // let cs_pin = pins.d5.into_output().downgrade();
    // let busy_in = pins.d6.into_pull_up_input().downgrade();
    // let dc = pins.d7.into_output().downgrade();
    // let rst = pins.d8.into_output().downgrade();

    // let epd = Epd1in54::new(
    //     &mut spi,
    //     cs_pin,
    //     busy_in,
    //     dc,
    //     rst,
    //     &mut arduino_hal::Delay::new(),
    // )
    // .unwrap();

    // let mut display: Display = Display::new(epd, spi);

    let mut button: Button;
    // let mut last_button: Button = Button::None;
    // let mut last_button_hold_time: Duration = Duration::ZERO;

    // let mut menu_state: MenuState = MenuState::Main;
    // let mut menu_state_timeout: Duration = Duration::ZERO;

    loop {
        button = Button::None;

        // if millis() - menu_state_timeout >= MENU_TIMEOUT {
        //     menu_state = MenuState::Main;
        //     update_display(&menu_state, &mut display);
        // }

        match dir_buttons.analog_read(&mut adc) {
            500...520 => button = Button::PressBottom,
            370...390 => button = Button::PressRight,
            180...200 => button = Button::PressLeft,
            330...340 => button = Button::PressTop,
            _ => {}
        }

        // sliding overwrites pressing
        match (
            poti_left_x.analog_read(&mut adc),
            poti_left_y.analog_read(&mut adc),
            poti_right_x.analog_read(&mut adc),
            adc.read_blocking(&adc::channel::ADC7),
        ) {
            (0.., 0...200, 0.., 900..) => button = Button::RotateRight,
            (0.., 900.., 0.., 0...200) => button = Button::RotateLeft,
            (0.., 800.., 0.., 800..) => button = Button::SlideDown,
            (0.., 0...200, 0.., 0...200) => button = Button::SlideUp,
            (900.., 0.., 0...200, 0..) => button = Button::SlideRight,
            (0...200, 0.., 900.., 0..) => button = Button::SlideLeft,
            _ => {}
        };
        uwriteln!(
            &mut serial,
            "dir_buttons: {:?}, \tpoti_left_x: {:?}, \tpoti_left_y: {:?}, \tpoti_right_x: {:?}, \tpoti_right_y: {:?}, \thand: {:?}, \tbutton: {:?}",
            dir_buttons.analog_read(&mut adc),
            poti_left_x.analog_read(&mut adc),
            poti_left_y.analog_read(&mut adc),
            poti_right_x.analog_read(&mut adc),
            adc.read_blocking(&adc::channel::ADC7),
            adc.read_blocking(&adc::channel::ADC6),
            button
        )
        .unwrap();
        delay_ms(100);
    }
}

fn update_display(menu_state: &MenuState, display: &mut Display) {
    let display_middle = Point {
        x: display.epd.width() as i32 / 2,
        y: display.epd.height() as i32 / 2,
    };
    let character_style = MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::On);
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    match menu_state {
        MenuState::Main => {
            Text::with_text_style("ALL", display_middle, character_style, text_style);
            display.display_frame();
        }
        MenuState::Lamp1 => {
            Text::with_text_style("1", display_middle, character_style, text_style);
            display.display_frame();
        }
        MenuState::Lamp2 => {
            Text::with_text_style("2", display_middle, character_style, text_style);
            display.display_frame();
        }
    }
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
