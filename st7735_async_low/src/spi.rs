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

//! Some traits needed to implement, in order to use [Commands](crate::Commands).

use core::future::Future;

/// Defines how the `DCX` pin operates.
///
/// According to the datasheet, `command` is LOW.
pub trait DcxPin {
    fn set_dcx_command_mode(&mut self);
    fn set_dcx_data_mode(&mut self);
}

/// Defines how a single [u8] is written with the `SCK` and `SDA` pins.
///
/// Common MCUs' SPI peripheral can be used, with
/// CPOL=1, CPHA=1 and MSB-first. Notice the timing requirement from ST7735's
/// datasheet.
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
