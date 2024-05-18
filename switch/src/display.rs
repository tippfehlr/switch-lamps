use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use epd_waveshare::{epd1in54::Epd1in54, prelude::*};

use arduino_hal::{
    clock::MHz16,
    hal::{
        delay::Delay,
        port::{PB0, PD5, PD6, PD7},
        Spi,
    },
    port::{
        mode::{Input, Output, PullUp},
        Pin,
    },
};

pub type Epd = Epd1in54<
    Spi,
    Pin<Output, PD5>,
    Pin<Input<PullUp>, PD6>,
    Pin<Output, PD7>,
    Pin<Output, PB0>,
    Delay<MHz16>,
>;
// the same with dynamic pins; use .downgrade() to convert. Comes with a performance penalty. not tested.
// type Epd = Epd1in54<Spi, Pin<Output>, Pin<Input<PullUp>>, Pin<Output>, Pin<Output>, Delay<MHz16>>;

pub enum DisplayError {}

pub struct Display {
    pub epd: Epd,
    spi: Spi,
    delay: Delay<MHz16>,
}

impl Display {
    pub fn new(epd: Epd, spi: Spi) -> Self {
        let delay = Delay::<MHz16>::new();
        Self { epd, spi, delay }
    }
    pub fn display_frame(&mut self) {
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
