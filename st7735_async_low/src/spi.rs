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

//! Some traits needed to implement, in order to use [`Commands`].
//!
//! The minimum would be to implement [DcxPin] and one of [WriteU8] and
//! [WriteU8s], then use an [`adapter`] to complete the missing
//! one of [WriteU8] and [WriteU8s]. With these, the write parts of [`Commands`]
//! are already usable.
//!
//! Note that the SPI protocol of ST7735's write commands
//! are actually compatible with command SPI implementations of
//! microcontrollers, eg., STM32 SPI with `CPOL=1` (clock idles at high) and
//! `CPHA=1` (data sampled at the second edge) without an SS pin. An example
//! can be found at [examples/stm32f3348_disco](https://github.com/jeru/st7735-async-low/tree/main/st7735_async_low/examples/stm32f3348_disco).
//!
//! If the user also needs to read data from the ST7735, then [Read] and
//! [ReadBits] should also be implemented. Presumably **no** high performance is
//! needed because reading is mostly for debugging purposes; the dummy bit
//! when reading 24- or 32-bit data is quite annoying (not totally impossible
//! to implement with hardware SPI but quite challenging); and when reading,
//! the clock must toggles slower than when writing (so the user needs to
//! reconfigure the SPI anyway). Therefore, it is
//! recommended that the user simply implements [ReadBits::read_bits()] with
//! bit-bangs.
//!
//! # Performance Consideration
//!
//! The reason to allow the user to implement [WriteU8] and [WriteU8s]
//! separately is for better performance. While it is natural to think
//! [WriteU8s] as a looped version of [WriteU8], there can be quite some
//! latency and throughput differences. Eg., in a STM32 microcontroller,
//! A loop-based [WriteU8s] is suboptimal for the following reasons:
//! * As soon as a byte is started to be sent, the user can already write
//!   the next byte to SPI's TX FIFO buffer. But [WriteU8::write_u8()] finishes
//!   only after the previous byte is fully sent to the device.
//! * DMA (direct memory access) is also very beneficial for [WriteU8s],
//!   especially when sending many bytes.
//!
//! So the user should only use an [`AdapterU8`] if they doesn't care
//! about the performance difference here.
//!
//! [`Commands`]: ../struct.Commands.html
//! [`adapter`]: ../adapters/index.html
//! [`AdapterU8`]: ../adapters/struct.AdapterU8.html

use core::future::Future;

/// Defines how the `DCX` pin operates.
pub trait DcxPin {
    /// Toggles the DCX pin to the `command mode` (LOW value).
    fn set_dcx_command_mode(&mut self);
    /// Toggles the DCX pin to the `data mode` (HIGH value).
    fn set_dcx_data_mode(&mut self);
}

/// Defines how a single [u8] is written with the `SCK` and `SDA` pins.
///
/// Common MCUs' SPI peripheral can be used, with
/// CPOL=1, CPHA=1 and MSB-first. Notice the timing requirement from ST7735's
/// datasheet. Most important ones:
/// * `SCK` low duration and high durations are at least 15ns long.
/// * `SCK` period is at least 66ns long.
pub trait WriteU8<'a> {
    type WriteU8Done : 'a + Future<Output=()>;

    fn write_u8(&'a mut self, data: u8) -> Self::WriteU8Done;
}

/// Defines how a sequence of `u8` or `u16` is written with the `SCK` and `SDA`
/// pins.
pub trait WriteU8s<'a> {
    type WriteU8sDone : 'a + Future<Output=()>;

    fn write_u8s(&'a mut self, data: &'a [u8]) -> Self::WriteU8sDone;
}

/// Defines how the MCU should use the `SCK` and `SDA` pins to read data.
///
/// It is assumed the reading isn't super important (mostly for debugging
/// purposes); and the implementation is actually quite hard because some
/// commands read without a dummy bit and some read with a dummy bit.
/// So the user can choose to simply **not** implement it. The write commands
/// of [Commands](crate::Commands) will still work in that case.
///
/// Calling `start_reading()` should switch the device into reading mode,
/// which should be switched back into writing mode when the returned object
/// of `start_reading()` is dropped.
pub trait Read<'a> {
    type ReadBitsType : 'a + for<'b> ReadBits<'b>;

    fn start_reading(&'a mut self) -> Self::ReadBitsType;
}

/// Defines how the helper RAII variable returned by [Read::start_reading()]
/// should behave.
///
/// Notice the timing requirement from ST7735's datasheet. Most important ones:
/// * `SCK` low duration and high durations are at least 60ns long.
/// * `SCK` period is at least 150ns long.
pub trait ReadBits<'a> {
    type ReadBitsDone : 'a + Future<Output=u32>;

    fn read_bits(&'a mut self, num_bits: usize) -> Self::ReadBitsDone;
}

#[cfg(test)]
mod test {
    use core::marker::PhantomData;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use super::*;

    struct FutureDummy1<'a, T, R = ()> {
        _t: &'a T,
        _r: PhantomData<R>,
    }
    impl<'a, T, R> FutureDummy1<'a, T, R> {
        pub fn new(t: &'a T) -> Self { Self{_t: t, _r: Default::default()} }
    }
    impl<'a, T, R: Default> Future for FutureDummy1<'a, T, R> {
        type Output = R;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<R> {
            Poll::Ready(Default::default())
        }
    }

    #[derive(Default)]
    struct Dummy1 { u: usize, i: isize }

    impl<'a> WriteU8<'a> for Dummy1 {
        type WriteU8Done = FutureDummy1<'a, usize>;

        fn write_u8(&'a mut self, _data: u8) -> Self::WriteU8Done {
            FutureDummy1::new(&self.u)
        }
    }

    impl<'a> WriteU8s<'a> for Dummy1 {
        type WriteU8sDone = FutureDummy1<'a, isize>;

        fn write_u8s(&'a mut self, _data: &'a [u8]) -> Self::WriteU8sDone {
            FutureDummy1::new(&self.i)
        }
    }

    #[test]
    fn write_u8() {
        let mut dummy: Dummy1 = Default::default();
        let _ = async { dummy.write_u8(10).await; };
    }

    #[test]
    fn write_u8_slice() {
        let mut dummy: Dummy1 = Default::default();
        let items: [u8; 3] = [0, 1, 2];
        let _ = async { dummy.write_u8s(&items).await; };
    }

    #[derive(Default)]
    struct Dummy2 { i: i64 }
    struct Dummy2Reader<'a> { d: &'a mut Dummy2 }

    impl<'a> Read<'a> for Dummy2 {
        type ReadBitsType = Dummy2Reader<'a>;

        fn start_reading(&'a mut self) -> Self::ReadBitsType {
            Dummy2Reader{d: self}
        }
    }

    impl<'a, 'b> ReadBits<'b> for Dummy2Reader<'a> {
        type ReadBitsDone = FutureDummy1<'b, i64, u32>;

        fn read_bits(&'b mut self, _num_bits: usize) -> Self::ReadBitsDone {
            FutureDummy1::new(&self.d.i)
        }
    }

    #[test]
    fn read_bits() {
        let mut dummy: Dummy2 = Default::default();
        let _ = async {
            let mut r = dummy.start_reading();
            r.read_bits(12).await
        };
    }
}
