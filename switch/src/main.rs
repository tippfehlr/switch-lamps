#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

mod display;
mod enums;
mod millis;

use display::Display;
use enums::{Button, MenuState};
use millis::{millis, millis_init};

use embedded_graphics::{
    mono_font::{iso_8859_9::FONT_9X15_BOLD, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use epd_waveshare::{epd1in54::Epd1in54, prelude::*};

use arduino_hal::{
    adc, delay_ms,
    hal::{delay::Delay, Atmega},
    usart::UsartOps,
    Usart,
};
use core::time::Duration;
use panic_halt as _;
use ufmt::uwriteln;

const CHANGE_PER_SECOND: i8 = 127;
const BUTTON_HOLD_FREQUENCY: i8 = 25;
const BUTTON_HOLD_INTERVAL: Duration = Duration::from_millis(1000 / BUTTON_HOLD_FREQUENCY as u64);
const MENU_TIMEOUT: Duration = Duration::from_secs(5);

// must not evaluate to 0
const CHANGE_PER_INTERVAL: i8 = CHANGE_PER_SECOND / BUTTON_HOLD_FREQUENCY;

// #[link_section = ".rodata"]
// static LAMP1: [u8; 200 * 200 / 8] = [];

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 115200);
    let mut adc = arduino_hal::Adc::new(dp.ADC, Default::default());
    millis_init(dp.TC0);

    unsafe { avr_device::interrupt::enable() };

    // this is the base voltage for the poti
    // when disabled, potis will not work
    let mut _poti_power = pins.d12.into_output_high();

    let dir_buttons = pins.a1.into_analog_input(&mut adc);
    let poti_left_x = pins.a0.into_analog_input(&mut adc);
    let poti_left_y = pins.a2.into_analog_input(&mut adc);
    let poti_right_x = pins.a3.into_analog_input(&mut adc);

    // write 6x 0x00 to signal a restart
    serial.write_byte(0x00);
    serial.write_byte(0x00);
    serial.write_byte(0x00);
    serial.write_byte(0x00);
    serial.write_byte(0x00);
    serial.write_byte(0x00);
    serial.write_byte(b'\n');

    // TODO: correct pins

    // let (mut spi, _) = arduino_hal::Spi::new(
    //     dp.SPI,
    //     pins.d13.into_output(),
    //     pins.d11.into_output(),
    //     pins.d4.into_pull_up_input(),
    //     pins.d10.into_output(),
    //     arduino_hal::spi::Settings::default(),
    // );
    // let cs_pin = pins.d5.into_output();
    // let busy_in = pins.d6.into_pull_up_input();
    // let dc = pins.d7.into_output();
    // let rst = pins.d8.into_output();

    // let pre_epd = Epd1in54::new(
    //     &mut spi,
    //     cs_pin,
    //     busy_in,
    //     dc,
    //     rst,
    //     &mut Delay::<MHz16>::new(),
    // );

    // uwriteln!(&mut serial, "{:#?}", pre_epd.is_err()).unwrap();

    // if pre_epd.is_err() {
    //     serial.write_byte(b'F');
    //     loop {}
    // }

    // let epd = pre_epd.unwrap();

    // let mut display: Display = Display::new(epd, spi);

    let mut button: Button;
    let mut last_button: Button = Button::None;
    let mut last_button_hold_time: Duration = Duration::ZERO;

    let mut menu_state: MenuState = MenuState::Main;
    let mut menu_state_timeout: Duration = Duration::ZERO;

    loop {
        button = Button::None;

        if millis() - menu_state_timeout >= MENU_TIMEOUT {
            menu_state = MenuState::Main;
            // update_display(&menu_state, &mut display);
        }

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

        if button != Button::None
            && (button != last_button || millis() - last_button_hold_time >= BUTTON_HOLD_INTERVAL)
        {
            last_button = button.clone();
            last_button_hold_time = millis();
            menu_state_timeout = millis();

            match button {
                Button::RotateRight => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    CHANGE_PER_INTERVAL,
                    0,
                    0,
                ),
                Button::RotateLeft => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    -CHANGE_PER_INTERVAL,
                    0,
                    0,
                ),
                Button::SlideUp => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    0,
                    CHANGE_PER_INTERVAL,
                    0,
                ),
                Button::SlideDown => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    0,
                    -CHANGE_PER_INTERVAL,
                    0,
                ),
                Button::SlideLeft => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    0,
                    0,
                    -CHANGE_PER_INTERVAL,
                ),
                Button::SlideRight => send_data(
                    &mut serial,
                    get_mask(&menu_state),
                    0,
                    0,
                    CHANGE_PER_INTERVAL,
                ),
                Button::PressTop => match &menu_state {
                    MenuState::Lamp1 => send_data(&mut serial, get_mask(&menu_state), -127, 0, 0),
                    _ => {
                        menu_state = MenuState::Lamp1;
                        menu_state_timeout = millis();

                        // update_display(&menu_state, &mut display);
                    }
                },
                Button::PressBottom => match menu_state {
                    MenuState::Lamp2 => send_data(&mut serial, get_mask(&menu_state), -127, 0, 0),
                    _ => {
                        menu_state = MenuState::Lamp2;
                        menu_state_timeout = millis();

                        // update_display(&menu_state, &mut display);
                    }
                },
                Button::PressLeft => {
                    increment_menu_state(&mut menu_state);

                    // update_display(&menu_state, &mut display);
                }
                Button::PressRight => {
                    decrement_menu_state(&mut menu_state);

                    // update_display(&menu_state, &mut display);
                }
                Button::None => unreachable!(),
            }
        }
        // uwriteln!(
        //     &mut serial,
        //     "dir_buttons: {:?}, poti_left_x: {:?}, poti_left_y: {:?}, poti_right_x: {:?}, poti_right_y: {:?}, hand: {:?}, button: {:?}",
        //     dir_buttons.analog_read(&mut adc),
        //     poti_left_x.analog_read(&mut adc),
        //     poti_left_y.analog_read(&mut adc),
        //     poti_right_x.analog_read(&mut adc),
        //     adc.read_blocking(&adc::channel::ADC7),
        //     adc.read_blocking(&adc::channel::ADC6),
        //     button
        // )
        // .unwrap();
        // delay_ms(1000);
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
            // display.epd.update_and_display_frame(&mut display.spi, buffer, &mut display.delay);
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
/// * `serial`: serial interface
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
    serial.write_byte(0x00);
    serial.write_byte((mask >> 8) as u8);
    serial.write_byte(mask as u8);
    serial.write_byte(brightness as u8);
    serial.write_byte(gamma as u8);
    serial.write_byte(position as u8);
    serial.write_byte(b'\n');
    serial.flush();
}
