#![no_std]
extern crate bitvec;
extern crate embedded_graphics;
extern crate embedded_hal as hal;

use bitvec::prelude::*;
use core::convert::TryInto;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{OriginDimensions, Size};
use embedded_graphics::Pixel;
use hal::blocking::spi::Write;
use hal::digital::v2::OutputPin;
use hal::spi::{Mode, Phase, Polarity}; // global logger

const MLCD_WR: u8 = 0x80; // write line command
const MLCD_CM: u8 = 0x20; // clear memory command
const MLCD_NO: u8 = 0x00; // nop command
const VCOM_HI: u8 = 0x40;
const VCOM_LO: u8 = 0x00;

pub const MODE: Mode = Mode {
    polarity: Polarity::IdleLow,
    phase: Phase::CaptureOnSecondTransition,
};

pub struct MemoryDisplay<SPI, CS, DISP> {
    spi: SPI,
    cs: CS,
    disp: DISP,
    buffer: [BitArr!(for 400, in u8, Lsb0); 240],
    vcom: bool,
}

impl<SPI, CS, DISP> OriginDimensions for MemoryDisplay<SPI, CS, DISP> {
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(400, 240)
    }
}

impl<SPI, CS, DISP, E> DrawTarget for MemoryDisplay<SPI, CS, DISP>
where
    SPI: Write<u8, Error = E>,
    CS: OutputPin,
    DISP: OutputPin,
{
    type Color = BinaryColor;
    type Error = E;

    fn draw_iter<T>(&mut self, item_pixels: T) -> Result<(), E>
    where
        T: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in item_pixels {
            if let Ok((x @ 0..=400, y @ 0..=240)) = coord.try_into() {
                self.set_pixel(x as u32, y as u32, color.is_on());
            }
        }
        Ok(())
    }
}

impl<SPI, CS, DISP, E> MemoryDisplay<SPI, CS, DISP>
where
    SPI: Write<u8, Error = E>,
    CS: OutputPin,
    DISP: OutputPin,
{
    /// Create a new MemoryDisplay object
    ///
    /// Issue a `clear` before drawing to the display
    pub fn new(spi: SPI, mut cs: CS, mut disp: DISP) -> Self {
        disp.set_low();
        cs.set_low();

        // The framebuffer: a byte-array for every line
        let buffer = [bitarr![u8, Lsb0; 0; 400]; 240];

        Self {
            spi,
            cs,
            disp,
            buffer,
            vcom: true,
        }
    }

    /// Enable the LCD
    pub fn enable(&mut self) {
        self.disp.set_high();
    }

    /// Disable the LCD
    pub fn disable(&mut self) {
        self.disp.set_low();
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, val: bool) {
        let line_buffer = &mut self.buffer[y as usize];
        line_buffer.set(x as usize, val);
    }

    /// Draw the buffer to the screen
    pub fn flush_buffer(&mut self) {
        defmt::debug!("Flushing framebuffer");
        self.cs.set_high();
        self.vcom = !self.vcom;

        // Write main message
        let vcom = if self.vcom { VCOM_HI } else { VCOM_LO };
        let _ = self.spi.write(&[MLCD_WR | vcom]);

        // Pack buffer into byte form and send
        //let mut buffer = [0; 52];
        for y in 0..240 {
            //            buffer[0] = reverse_bits::msb2lsb(i + 1);
            //            buffer[1..51].clone_from_slice(&self.buffer[i as usize][0..50]);
            // Write line number (starting at 1)
            let line_no = y + 1;
            let line_no_bits_msb = BitSlice::<u8, Lsb0>::from_element(&line_no);
            let line_no_bits = Self::swap(line_no_bits_msb);
            if line_no == 3 {
                assert!(line_no_bits == 0b11000000);
            }

            let line_buffer_msb = self.buffer[y as usize];

            // Local write buffer for this line: line number, data (50 bits), 8-bit trailer
            let mut write_buffer = [0u8; 52];
            write_buffer[0] = line_no_bits;

            let mut chunks = line_buffer_msb.chunks(8);
            (1..51).for_each(|x| {
                write_buffer[x] = Self::swap(chunks.next().unwrap());
            });
            write_buffer[51] = MLCD_NO;
            let _ = self.spi.write(&write_buffer);
        }

        // Write the 16-bit frame trailer
        let _ = self.spi.write(&[MLCD_NO, MLCD_NO]);

        self.cs.set_low();
    }

    pub fn swap(byte: &BitSlice<u8, Lsb0>) -> u8 {
        // View slice with most-significant bit first (inverted)
        let mut local_buffer = bitarr!(u8, Msb0; 0; 8);
        for (i, bit) in byte.iter().by_ref().enumerate() {
            local_buffer.set(i, *bit);
        }
        local_buffer.load::<u8>()
    }

    pub fn clear_buffer(&mut self) {
        for y in 0..240 {
            let line_buffer = &mut self.buffer[y];
            line_buffer.fill(true);
        }
    }

    /// Clear the screen and the buffer
    pub fn clear(&mut self) {
        self.clear_buffer();
        self.write_spi(&[MLCD_CM, MLCD_NO]);
    }

    /// Enter display mode for power savings
    pub fn display_mode(&mut self) {
        self.write_spi(&[0x00, 0x00]);
    }

    /// Internal function for handling the chip select
    fn write_spi(&mut self, data: &[u8]) {
        self.cs.set_high();

        let _ = self.spi.write(data);

        self.cs.set_low();
    }
}
