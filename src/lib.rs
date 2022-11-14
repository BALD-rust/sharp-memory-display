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
use core::ops::{BitOr, Not};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{OriginDimensions, Size};
use embedded_graphics::Pixel;
use hal::blocking::spi::Write;
use hal::digital::v2::OutputPin;
use hal::spi::Mode;

#[cfg(not(any(
    feature = "ls027b7dh01",
    feature = "ls012b7dd06",
    feature = "ls010b7dh04",
    feature = "ls013b7dh05",
    feature = "ls011b7dh03",
)))]
compile_error!("Please specify a display type via the feature flag");

// Pull in the appropriate set of constants for the particular model of display
#[cfg_attr(feature = "ls027b7dh01", path = "ls027b7dh01.rs")]
#[cfg_attr(feature = "ls012b7dd06", path = "ls012b7dd06.rs")]
#[cfg_attr(feature = "ls010b7dh04", path = "ls010b7dh04.rs")]
#[cfg_attr(feature = "ls013b7dh05", path = "ls013b7dh05.rs")]
#[cfg_attr(feature = "ls011b7dh03", path = "ls011b7dh03.rs")]
mod display;

const DUMMY_DATA: u8 = 0x00; // This can really be anything, but the spec sheet recommends 0s

#[derive(Clone, Copy, PartialEq, Eq)]
enum Vcom {
    // For details see the document https://www.sharpsde.com/fileadmin/products/Displays/2016_SDE_App_Note_for_Memory_LCD_programming_V1.3.pdf
    Lo = 0x00, // 0b_0______ M1 == 0
    Hi = 0x40, // 0b_1______ M1 == 1
}

impl Not for Vcom {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Vcom::Lo => Vcom::Hi,
            Vcom::Hi => Vcom::Lo,
        }
    }
}

impl BitOr<Command> for Vcom {
    type Output = u8;

    fn bitor(self, rhs: Command) -> Self::Output {
        (self as u8) | (rhs as u8)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    // For details see the document https://www.sharpsde.com/fileadmin/products/Displays/2016_SDE_App_Note_for_Memory_LCD_programming_V1.3.pdf
    Nop = 0x00,         // 0b0_0_____ M0 == 0, M2 == 0
    ClearMemory = 0x20, // 0b0_1_____ M0 == 0, M2 == 1
    WriteLine = 0x80,   // 0b1_0_____ M0 == 1, M2 == 0
}

impl BitOr<Vcom> for Command {
    type Output = u8;

    fn bitor(self, rhs: Vcom) -> Self::Output {
        (self as u8) | (rhs as u8)
    }
}

/// Mode to configure the SPI device in in order to communicate with the display.
pub const MODE: Mode = display::MODE;

// Local write buffer size for a line: line number, then data (e.g. 400px / 8 bits = 50 bytes), followed by 8-bit trailer
const WRITE_BUFFER_SIZE: usize = (display::WIDTH / 8) + 2;

pub struct MemoryDisplay<SPI, CS, DISP> {
    spi: SPI,
    cs: CS,
    disp: DISP,
    buffer: [BitArr!(for display::WIDTH, in u8, Lsb0); display::HEIGHT],
    touched: BitArr!(for display::HEIGHT, in u8, Lsb0),
    vcom: Vcom,
    clear_state: BinaryColor,
}

impl<SPI, CS, DISP> OriginDimensions for MemoryDisplay<SPI, CS, DISP> {
    fn size(&self) -> Size {
        Size::new(display::WIDTH as u32, display::HEIGHT as u32)
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
            if coord.x < 0 || coord.x >= (display::WIDTH as i32) || coord.y < 0 || coord.y >= (display::HEIGHT as i32) {
                // Ignore attempts to draw outside of display bounds, continue to next pixel
                continue
            } else {
                unsafe { self.set_pixel(coord.x as u32, coord.y as u32, color) };
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
        let buffer = [bitarr![u8, Lsb0; 0; display::WIDTH]; display::HEIGHT];
        let touched = bitarr![u8, Lsb0; 0; display::HEIGHT];

        Self {
            spi,
            cs,
            disp,
            buffer,
            touched,
            vcom: Vcom::Hi,
            clear_state: BinaryColor::On,
        }
    }

    /// Set the value that screen buffer should be set to when issuing a clear command.
    /// Note that this might be different from the state the hardware will set itself to.
    /// You'll need to execute a flush_buffer following the call to clear if the
    /// desired state differs from the default one in the hardware.
    pub fn set_clear_state(&mut self, clear_state: BinaryColor) {
        self.clear_state = clear_state;
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
    ///
    /// N.B. This function does no bounds checking! Attempting to draw
    /// to a location outside the bounds of the display will result in
    /// a panic.
    pub unsafe fn set_pixel(&mut self, x: u32, y: u32, val: BinaryColor) {
        let line_buffer = &mut self.buffer[y as usize];
        line_buffer.set(x as usize, val.is_on());
        self.touched.set(y as usize, true);
    }

    /// Draw all lines of the buffer to the screen which have changed since last calling this
    /// function.
    pub fn flush_buffer(&mut self) {
        let _ = self.cs.set_high();

        self.vcom = !self.vcom;
        let _ = self.spi.write(&[Command::WriteLine | self.vcom]);

        // Pack buffer into byte form and send
        for y in self.touched.iter_ones() {
            // Known problem with BitArr where if it's length isn't exactly divisible by the underlying storage size
            // it will return indexes greater than its length. Break loop early if we've exceeded the size of buffer.
            // https://github.com/bitvecto-rs/bitvec/issues/159 for details.
            if y >= self.buffer.len() {
                break;
            }
            // Write line number (starting at 1)
            let line_no = (y + 1) as u8;
            defmt::trace!("Writing line {}", line_no);
            let line_no_bits_msb = BitSlice::<u8, Lsb0>::from_element(&line_no);
            let line_no_bits = Self::swap(line_no_bits_msb);

            let line_buffer_msb = self.buffer[y as usize];

            let mut write_buffer = [0u8; WRITE_BUFFER_SIZE];
            write_buffer[0] = line_no_bits;

            let mut chunks = line_buffer_msb.chunks(8);
            (1..(write_buffer.len() - 1)).for_each(|x| {
                write_buffer[x] = Self::swap(chunks.next().unwrap());
            });
            // Technically this is supposed to be part of the address of the following line, but we'll just send it here because it's easier
            write_buffer[write_buffer.len() - 1] = DUMMY_DATA;
            let _ = self.spi.write(&write_buffer);
        }

        // Write the 16-bit frame trailer (first 8 bits come from the end of the last line written)
        let _ = self.spi.write(&[DUMMY_DATA]);

        let _ = self.cs.set_low();

        self.touched.fill(false);
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
            line_buffer.fill(self.clear_state.is_on());
        }
        self.touched.fill(true);
    }

    /// Clear the screen and the internal framebuffer.
    pub fn clear(&mut self) {
        self.clear_buffer();
        self.vcom = !self.vcom;
        self.write_spi(&[Command::ClearMemory | self.vcom, DUMMY_DATA]);
    }

    /// Puts the display into power saving mode. This can also be used to send
    /// the VCOM signal which Sharp recommends sending at least once a second.
    /// No actual harm seems to come from failing to do so however.
    pub fn display_mode(&mut self) {
        self.vcom = !self.vcom;
        self.write_spi(&[Command::Nop | self.vcom, DUMMY_DATA]);
    }

    /// Internal function for handling the chip select
    fn write_spi(&mut self, data: &[u8]) {
        let _ = self.cs.set_high();

        let _ = self.spi.write(data);

        let _ = self.cs.set_low();
    }
}
