use arduino_hal::pac;

pub fn configure_int0(eicra: &pac::exint::EICRA) {
    // 0x00: The low level of INT0 generates an interrupt reques
    // 0x01: Any logical change on INT0 generates an interrupt request.
    // 0x02: The falling edge of INT0 generates an interrupt request.
    // 0x03: The rising edge of INT0 generates an interrupt request.
    // 0x02 and 0x03 need a timer and don’t work in powerdown.
    // see page 54.
    eicra.write(|w| w.isc0().bits(0x01));
}

pub fn set_sleep_mode(smcr: &pac::cpu::SMCR) {
    // set sleep mode to powerdown; see page 34 for sleep modes.
    smcr.write(|w| w.sm().pdown())
}

pub fn enter_sleep(eimsk: &pac::exint::EIMSK, smcr: &pac::cpu::SMCR) {
    // enable INT0
    eimsk.write(|w| w.int0().set_bit());
    // set sleep enable bit
    smcr.write(|w| w.se().set_bit());
    // go to sleep
    avr_device::asm::sleep();

    // sleeping here; continuing once INT0 triggers.

    // clear sleep enable bit
    smcr.write(|w| w.se().clear_bit());
    // disable INT0
    eimsk.write(|w| w.int0().clear_bit());
}

#[allow(non_snake_case)]
#[avr_device::interrupt(atmega328p)]
fn INT0() {}
