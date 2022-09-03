# `sharp-memory-display`
![Crates.io](https://img.shields.io/crates/v/sharp-memory-display?style=flat-square)
![Crates.io](https://img.shields.io/crates/l/sharp-memory-display?style=flat-square)

## Summary
`sharp-memory-display` provides an `embedded-graphics` driver for SHARP memory-in-pixel displays, such as the LS027B7DH01 and similar models. The crate depends on `embedded-hal` and is `#![no_std]`-compatible.

## Usage
Just depend on the crate in your Cargo manifest, specifying your display model via `features`:
```
[dependencies]
sharp-memory-display = { version: "0.1", features: ["ls027b7dh01"] }
```

Now you can bring the crate into scope:
```
use sharp_memory_display::*
```

Then simply construct a new `MemoryDisplay` from an SPI struct, a chip-select pin, and a display pin similar to this:

```
let cs = pins.pa14.into_push_pull_output();
let disp = pins.pa16.into_push_pull_output();
let spi = spi::Config::new(&peripherals.MCLK, spi_sercom, pads, freq)
    .baud(Hertz(500_000u32))
    .spi_mode(sharp_memory_display::MODE)
    .enable();

// Create display
let mut disp = MemoryDisplay::new(spi, cs, disp)
```

Please note the maximum supported SPI baud rate for your display. You can find it in the corresponding [datasheet](https://www.sharpsde.com/products/displays/memory-lcd/).

**Note:** You must specify your display via `features`. Supported display models currently are:
 - `ls027b7dh01` (tested)
 - `ls013b7dh05`
 - `ls012b7dd06`
 - `ls010b7dh04`

Support for additional display models can be easily added. Merge requests are welcome :)

## Bug Reports and Feature Requests
Contributions to this project are welcome. You can find the [issue tracker](https://todo.sr.ht/~doesnotcompete/sharp-memory-display) and the [code repository](https://git.sr.ht/~doesnotcompete/sharp-memory-display) at sourcehut. You may also submit bug reports or feature requests via email to [~doesnotcompete/sharp-memory-display@todo.sr.ht](mailto:~doesnotcompete/sharp-memory-display@todo.sr.ht).
