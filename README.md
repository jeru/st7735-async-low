# A low-level async library for ST7735 LCD in Rust

This is a low-level library to implement ST7735 commands as close as their
original forms but in the style of Rust's "async/await" paradigm.

## How to use

An example is at the `examples/stm32f3348_disco" directory. In general, the user
should implement the traits under `crate::spi` with their MCU, then wrap the
implementation with `crate::Commands`, which provides the ST7735 commands in
their original names, as defined in the datasheet.

TODO: Add commandline-level instruction after the project is published to
crates.io.
