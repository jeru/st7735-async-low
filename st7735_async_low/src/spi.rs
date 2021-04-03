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
///
/// # Examples
///
/// ```
/// # #![allow(incomplete_features)]
/// #![feature(generic_associated_types)]
/// #![feature(min_type_alias_impl_trait)]
/// # use st7735_async_low::spi::*;
/// extern crate async_trait_static;
/// use core::future::Future;
///
/// struct MySpi { /*...*/ }
///
/// #[async_trait_static::ritit]
/// impl WriteU8 for MySpi {
///     fn write_u8(&mut self, data: u8) -> impl Future<Output=()> {
///         # let _ = data;
///         async move { /*...*/ }
///     }
/// }
/// ```
#[async_trait_static::ritit]
pub trait WriteU8 {
    fn write_u8(&mut self, data: u8) -> impl Future<Output=()>;
}

/// Defines how a sequence of `u8` or `u16` is written with the `SCK` and `SDA`
/// pins.
///
/// It is assumed that the struct to implement this trait has already
/// implemented [WriteU8].
///
/// # Examples
/// Option 1: Directly implement.
///
/// ```
/// # #![allow(incomplete_features)]
/// # #![feature(generic_associated_types)]
/// # #![feature(min_type_alias_impl_trait)]
/// # use st7735_async_low::spi::*;
/// # extern crate async_trait_static;
/// # use core::future::Future;
/// #
/// # struct MySpi { /*...*/ }
/// #
/// # #[async_trait_static::ritit]
/// # impl WriteU8 for MySpi {
/// #     fn write_u8(&mut self, data: u8) -> impl Future<Output=()> {
/// #         let _ = data;
/// #         async move { /*...*/ }
/// #     }
/// # }
/// // Following the example of `WriteU8`.
/// #[async_trait_static::ritit]
/// impl WriteBatch for MySpi {
///     fn write_u8_iter<I: Iterator<Item=u8>>(&mut self, iter: I)
///             -> impl Future<Output=()> {
///         async move { /*...*/ }
///     }
///     fn write_u16_iter<I: Iterator<Item=u16>>(&mut self, iter: I)
///             -> impl Future<Output=()> {
///         async move { /*...*/ }
///     }
/// }
/// // Then use `MySpi` as `impl WriteBatch` somehow.
/// ```
///
/// Option 2: Use [crate::adapters::AdapterU8].
///
/// # Performance Considerations
///
/// When the run-time performance is not critical, the user can choose to
/// only implement [WriteU8], then use [AdapterU8](crate::AdapterU8) to
/// get a default implementation of [WriteBatch]. But notice the inefficiency
/// of such default implementation: it only start trying to send the the next
/// byte until the previous byte is finished.
///
/// In many MCUs with SPI peripherals, better performance (both lower latency
/// and lower CPU usage) can be achieved via more careful timing.
/// Eg., in some STM32 MCUs, the next byte can be written to the buffer register
/// as soon as the hardware starts sending the previous byte -- remarkly, no
/// need to wait for the previous byte to fully finish.
/// DMA is also a good option when sending more bytes in a batch.
/// These benefits can be adapted via alternative implementations of
/// `WriteBatch`.
#[async_trait_static::ritit]
pub trait WriteBatch : WriteU8 {
    fn write_u8_iter<I: Iterator<Item=u8>>(&mut self, iter: I)
        -> impl Future<Output=()>;
    fn write_u16_iter<I: Iterator<Item=u16>>(&mut self, iter: I)
        -> impl Future<Output=()>;
}

/// Convenient function with broader input types.
#[inline(always)]
pub fn write_u8s<'a, S, I: 'a>(spi: &'a mut S, items: I)
    -> impl Future<Output=()> + 'a
where S: WriteBatch,
      I: IntoIterator<Item=&'a u8> {
    spi.write_u8_iter(items.into_iter().copied())
}

/// Convenient function with broader input types.
#[inline(always)]
pub fn write_u16s<'a, S, I: 'a>(spi: &'a mut S, items: I)
    -> impl Future<Output=()> + 'a
where S: WriteBatch,
      I: IntoIterator<Item=&'a u16> {
    spi.write_u16_iter(items.into_iter().copied())
}

// TODO: shouldn't need to separate this from `ReadU8` after
// async_trait_static supports mixed setup.
/// Defines how the MCU should switch the SPI between the reading and writing
/// modes.
pub trait ReadModeSetter {
    fn start_reading(&mut self);
    fn finish_reading(&mut self);
}

/// Defines how the MCU should use the `SCK` and `SDA` pins to read data.
///
/// It is assumed the reading isn't super important (mostly for debugging
/// purposes); and the implementation is actually quite hard because some
/// commands read without a dummy bit and some read with a dummy bit.
/// So the user can choose to simply **not** implement it. The write commands
/// of [Commands](crate::Commands) will still work in that case.
///
/// # Example
///
/// ```
/// # #![allow(incomplete_features)]
/// #![feature(generic_associated_types)]
/// #![feature(min_type_alias_impl_trait)]
/// # use st7735_async_low::spi::*;
/// extern crate async_trait_static;
/// use core::future::Future;
///
/// struct MySpi { /*...*/ }
/// # impl ReadModeSetter for MySpi {
/// #     fn start_reading(&mut self) {}
/// #     fn finish_reading(&mut self) {}
/// # }
///
/// #[async_trait_static::ritit]
/// impl Read for MySpi {
///     fn read(&mut self, num_bits: usize) -> impl Future<Output=u32> {
///         async move {
///             let mut r = 0u32;
///             for _ in 0..num_bits {
///                 let bit = /*...read a bit...*/
///                 # 0u32;
///                 r = r.wrapping_shl(1) | bit;
///             }
///             r
///         }
///     }
/// }
/// ```
#[async_trait_static::ritit]
pub trait Read : ReadModeSetter {
    /// Supposed to bit-bang so many bits.
    fn read(&mut self, num_bits: usize) -> impl Future<Output=u32>;
}

#[cfg(test)]
mod test {
    use super::*;

    struct Dummy1;
    #[async_trait_static::async_trait]
    impl WriteU8 for Dummy1 {
        async fn write_u8(&mut self, _data: u8) {}
    }

    #[async_trait_static::ritit]
    impl WriteBatch for Dummy1 {
        fn write_u8_iter<I: Iterator<Item=u8>>(&mut self, iter: I)
                -> impl Future<Output=()> {
            async move {
                let mut _sum: u8 = 0;
                for v in iter { _sum += v; }
            }
        }
        fn write_u16_iter<I: Iterator<Item=u16>>(&mut self, iter: I)
                -> impl Future<Output=()> {
            async move {
                let mut _sum: u16 = 0;
                for v in iter { _sum += v; }
            }
        }
    }

    #[test]
    fn write_u8_iter_with_slice() {
        let mut dummy = Dummy1{};
        let items: [u8; 3] = [0, 1, 2];
        let _ = async { write_u8s(&mut dummy, &items).await; };
    }

    #[test]
    fn write_u16_iter_with_slice() {
        let mut dummy = Dummy1{};
        let items: [u16; 3] = [0, 1, 2];
        let _ = async { write_u16s(&mut dummy, &items).await; };
    }
}
