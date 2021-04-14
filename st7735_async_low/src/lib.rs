// Copyright 2021 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This crates aims to provide the native ST7735 commands in their original
//! form, thus is a low-level library.
//!
//! A user of this crate should implement the write traits in [crate::spi], then
//! wrap it with [Commands](crate::Commands) to use the commands. An example can
//! be found at the [examples/stm32f3348_disco](https://github.com/jeru/st7735-async-low/tree/main/st7735_async_low/examples/stm32f3348_disco)
//! directory of the crate.

#![no_std]

#[cfg(test)] extern crate std;
#[cfg(test)] extern crate tokio;
#[cfg(test)] extern crate mockall;

pub mod adapters;
mod command_structs;
pub use command_structs::{
    Colmod, ColorComponentOrder, ColumnOrder, Madctl, RowColumnSwap, RowOrder};
mod commands;
pub use commands::{Commands, RamWriter};
pub mod spi;

#[cfg(test)] pub mod testing_device;
