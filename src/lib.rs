/*  This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

//! Support for SHARP memory-in-pixel display devices via [`embedded_graphics`].
//!
//! # Usage                                                                                  
//! Create a new [`MemoryDisplay`] and simply use it as an [`embedded_graphics`]
//! [`embedded_graphics::draw_target::DrawTarget`].
//! You must flush the framebuffer with [`MemoryDisplay::flush_buffer`] for the buffer to be written to the
//! screen.
//!
//! Please specify one of the supported displays via the Cargo `feature` flag. This sets
//! appropriate buffer and target sizes for the device at compile time.
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

#[cfg(not(any(
    feature = "ls027b7dh01",
    feature = "ls012b7dd06",
    feature = "ls010b7dh04",
    feature = "ls013b7dh05"
)))]
compile_error!("Please specify a display type via the feature flag");

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
    #[cfg(feature = "ls027b7dh01")]
    buffer: [BitArr!(for 400, in u8, Lsb0); 240],
    #[cfg(feature = "ls012b7dd06")]
    buffer: [BitArr!(for 240, in u8, Lsb0); 240],
    #[cfg(feature = "ls010b7dh04")]
    buffer: [BitArr!(for 128, in u8, Lsb0); 128],
    #[cfg(feature = "ls013b7dh05")]
    buffer: [BitArr!(for 144, in u8, Lsb0); 168],
    vcom: bool,
}

impl<SPI, CS, DISP> OriginDimensions for MemoryDisplay<SPI, CS, DISP> {
    #[cfg(feature = "ls027b7dh01")]
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(400, 240)
    }
    #[cfg(feature = "ls012b7dd06")]
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(240, 240)
    }
    #[cfg(feature = "ls010b7dh04")]
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(128, 128)
    }
    #[cfg(feature = "ls013b7dh05")]
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size::new(144, 168)
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
        let y_min = (self.size().height + 1) as u32;
        let (mut y_min, mut y_max) = (y_min, 0u32);

        for Pixel(coord, color) in item_pixels {
            #[cfg(feature = "ls027b7dh01")]
            if let Ok((x @ 0..=400, y @ 0..=240)) = coord.try_into() {
                self.set_pixel(x as u32, y as u32, color.is_on());
                if y < y_min {
                    y_min = y
                }
                if y > y_max {
                    y_max = y
                }
            }
            #[cfg(feature = "ls012b7dd06")]
            if let Ok((x @ 0..=240, y @ 0..=240)) = coord.try_into() {
                self.set_pixel(x as u32, y as u32, color.is_on());
                if y < y_min {
                    y_min = y
                }
                if y > y_max {
                    y_max = y
                }
            }
            #[cfg(feature = "ls010b7dh04")]
            if let Ok((x @ 0..=128, y @ 0..=128)) = coord.try_into() {
                self.set_pixel(x as u32, y as u32, color.is_on());
                if y < y_min {
                    y_min = y
                }
                if y > y_max {
                    y_max = y
                }
            }
            #[cfg(feature = "ls013b7dh05")]
            if let Ok((x @ 0..=144, y @ 0..=168)) = coord.try_into() {
                self.set_pixel(x as u32, y as u32, color.is_on());
                if y < y_min {
                    y_min = y
                }
                if y > y_max {
                    y_max = y
                }
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
    /// Create a new instance of `MemoryDisplay`.
    ///
    /// Please issue a `clear` before drawing to the display.
    pub fn new(spi: SPI, mut cs: CS, mut disp: DISP) -> Self {
        let _ = disp.set_low();
        let _ = cs.set_low();

        // The framebuffer: a byte-array for every line
        #[cfg(feature = "ls027b7dh01")]
        let buffer = [bitarr![u8, Lsb0; 0; 400]; 240];
        #[cfg(feature = "ls012b7dd06")]
        let buffer = [bitarr![u8, Lsb0; 0; 240]; 240];
        #[cfg(feature = "ls010b7dh04")]
        let buffer = [bitarr![u8, Lsb0; 0; 128]; 128];
        #[cfg(feature = "ls013b7dh05")]
        let buffer = [bitarr![u8, Lsb0; 0; 144]; 168];

        Self {
            spi,
            cs,
            disp,
            buffer,
            vcom: true,
        }
    }

    /// Enable the LCD by driving the display pin high.
    pub fn enable(&mut self) {
        let _ = self.disp.set_high();
    }

    /// Disable the LCD.
    pub fn disable(&mut self) {
        let _ = self.disp.set_low();
    }

    /// Sets a single pixel value in the internal framebuffer.
    pub fn set_pixel(&mut self, x: u32, y: u32, val: bool) {
        let line_buffer = &mut self.buffer[y as usize];
        line_buffer.set(x as usize, val);
    }

    /// Draw the buffer to the screen. This function only updates the vertical section of the screen specified by `line_start` and `line_stop`.
    ///
    /// * `line_start` - The first line (y index) to update.
    /// * `line_stop` - The last line to update.
    pub fn flush_buffer(&mut self, line_start: u8, line_stop: u8) {
        let _ = self.cs.set_high();
        self.vcom = !self.vcom;

        // Write main message
        let vcom = if self.vcom { VCOM_HI } else { VCOM_LO };
        let _ = self.spi.write(&[MLCD_WR | vcom]);

        // Pack buffer into byte form and send
        defmt::trace!("Flushing {} to {}", line_start, line_stop);
        for y in line_start..line_stop {
            // Write line number (starting at 1)
            let line_no = y + 1;
            let line_no_bits_msb = BitSlice::<u8, Lsb0>::from_element(&line_no);
            let line_no_bits = Self::swap(line_no_bits_msb);

            let line_buffer_msb = self.buffer[y as usize];

            // Local write buffer for this line: line number, then data (e.g. 400px / 8 bits = 50 bytes), followed by 8-bit trailer
            #[cfg(feature = "ls027b7dh01")]
            let mut write_buffer = [0u8; 52];
            #[cfg(feature = "ls027b7dd06")]
            let mut write_buffer = [0u8; 32];
            #[cfg(feature = "ls010b7dh04")]
            let mut write_buffer = [0u8; 18];
            #[cfg(feature = "ls013b7dh05")]
            let mut write_buffer = [0u8; 20];
            write_buffer[0] = line_no_bits;

            let mut chunks = line_buffer_msb.chunks(8);
            (1..(write_buffer.len() - 1)).for_each(|x| {
                write_buffer[x] = Self::swap(chunks.next().unwrap());
            });
            write_buffer[write_buffer.len() - 1] = MLCD_NO;
            let _ = self.spi.write(&write_buffer);
        }

        // Write the 16-bit frame trailer
        let _ = self.spi.write(&[MLCD_NO, MLCD_NO]);

        let _ = self.cs.set_low();
    }

    /// Contrary to the MSB order most SPI devices use, the memory-in-pixel displays use LSB byte
    /// order. This function swaps the order of a single byte (viewed via a `BitSlice`) and converts it to `u8`.
    pub fn swap(byte: &BitSlice<u8, Lsb0>) -> u8 {
        // View slice with most-significant bit first (inverted)
        let mut local_buffer = bitarr!(u8, Msb0; 0; 8);
        for (i, bit) in byte.iter().by_ref().enumerate() {
            local_buffer.set(i, *bit);
        }
        local_buffer.load::<u8>()
    }

    /// Clear just the internal framebuffer, without writing changes to the display.
    pub fn clear_buffer(&mut self) {
        for y in 0..(self.size().height as usize) {
            let line_buffer = &mut self.buffer[y];
            line_buffer.fill(true);
        }
    }

    /// Clear the screen and the internal framebuffer.
    pub fn clear(&mut self) {
        self.clear_buffer();
        self.write_spi(&[MLCD_CM, MLCD_NO]);
    }

    /// Internal function for handling the chip select
    fn write_spi(&mut self, data: &[u8]) {
        let _ = self.cs.set_high();

        let _ = self.spi.write(data);

        let _ = self.cs.set_low();
    }
}
