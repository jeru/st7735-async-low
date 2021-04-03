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

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::spi;
use spi::{DcxPin, Read, WriteU8, WriteU8s};

/// A helper to add [WriteU8s] support when [WriteU8] is implemented.
///
/// Supposedly **not** very efficient. See the Performance Consideration section
/// of the module [spi].
pub struct AdapterU8<W> { w: W }

impl<W> AdapterU8<W> {
    pub fn new(w: W) -> Self { Self{w} }
}

impl<W: DcxPin> DcxPin for AdapterU8<W> {
    fn set_dcx_command_mode(&mut self) { self.w.set_dcx_command_mode(); }
    fn set_dcx_data_mode(&mut self) { self.w.set_dcx_data_mode(); }
}

impl<'a, W: Read<'a>> Read<'a> for AdapterU8<W> {
    type ReadBitsType = <W as Read<'a>>::ReadBitsType;

    fn start_reading(&'a mut self) -> Self::ReadBitsType {
        self.w.start_reading()
    }
}

impl<'a, W: WriteU8<'a>> WriteU8<'a> for AdapterU8<W> {
    type WriteU8Done = <W as WriteU8<'a>>::WriteU8Done;

    fn write_u8(&'a mut self, data: u8) -> Self::WriteU8Done {
        self.w.write_u8(data)
    }
}

impl<'a, W: 'a> WriteU8s<'a> for AdapterU8<W> where for<'w> W: WriteU8<'w> {
    type WriteU8sDone = RepeatU8<'a, W>;

    fn write_u8s(&'a mut self, data: &'a [u8]) -> Self::WriteU8sDone {
        RepeatU8{data: data, w: &mut self.w, current_write: None}
    }
}

pub struct RepeatU8<'a, W: for<'w> WriteU8<'w>> {
    data: &'a [u8],
    // Lifetime is also 'a. `current_write` when not `None` can actually borrow
    // `*w` in mut.
    w: *mut W,
    current_write: Option<<W as WriteU8<'a>>::WriteU8Done>,
}

impl<'a, W: 'a + for<'w> WriteU8<'w>> Future for RepeatU8<'a, W> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // Safety: Only `Self::current_write` needs pinning. The implementation
        // below indeed never moves it, only creates and drops.
        let ru = unsafe {self.get_unchecked_mut()};
        loop {
            if ru.current_write.is_none() {
                if let Some((first, remaining)) = ru.data.split_first() {
                    // Safety: `current_write` is `None`.
                    let w: &'a mut W = unsafe {&mut *ru.w};
                    ru.current_write = Some(w.write_u8(*first));
                    ru.data = remaining;
                } else {
                    return Poll::Ready(());
                }
            }
            if let Some(ref mut done) = &mut ru.current_write {
                // Safety: Pinning a field of a pinned.
                let done = unsafe {Pin::new_unchecked(done)};
                if done.poll(cx).is_pending() {
                    return Poll::Pending;
                }
            } else {
                unsafe {core::hint::unreachable_unchecked()};
            }
            ru.current_write = None;
        }
    }
}

#[cfg(test)]
mod adapter_u8_tests {
    use mockall::Sequence;
    use mockall::predicate::eq;

    use crate::spi::ReadBits as _;
    use crate::testing_device::{block_on, MockDevice};
    use super::*;

    #[test]
    fn write_u8_as_is() {
        let mut a = AdapterU8::new(MockDevice::new());
        a.set_dcx_command_mode();
        a.w.mock().expect_write_command()
            .with(eq(0x34))
            .times(1);
        block_on(a.write_u8(0x34));
    }

    #[test]
    fn write_u8s() {
        let mut a = AdapterU8::new(MockDevice::new());
        a.set_dcx_data_mode();
        let mut seq = mockall::Sequence::new();
        a.w.mock().expect_write_data()
            .with(eq(0x34))
            .times(1)
            .in_sequence(&mut seq);
        a.w.mock().expect_write_data()
            .with(eq(0x56))
            .times(1)
            .in_sequence(&mut seq);
        a.w.mock().expect_write_data()
            .with(eq(0x12))
            .times(1)
            .in_sequence(&mut seq);
        block_on(a.write_u8s(&[0x34, 0x56, 0x12]));
    }

    #[test]
    fn read_as_is() {
        let src: u32 = 0b111010;
        let src_len: usize = 6;

        let mut a = AdapterU8::new(MockDevice::new());
        let mut seq = Sequence::new();
        a.w.mock().expect_start_reading().times(1).in_sequence(&mut seq);
        for i in (0..src_len).rev() {
            let bit = src >> i & 1 != 0;
            a.w.mock().expect_read_bit()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move || bit);
        }
        a.w.mock().expect_finish_reading().times(1).in_sequence(&mut seq);

        let value = block_on(a.start_reading().read_bits(src_len));
        assert_eq!(value, src);
    }
}  // mod adapter_u8_tests

/// A helper to add [WriteU8] support when [WriteU8s] is implemented.
///
/// There is a slightly overhead on using an array to represent an element,
/// especially when the compiler fails to inline the functions. The user
/// should decide on their own whether they need to implement [WriteU8] and
/// [WriteU8s] individually.
pub struct AdapterU8s<W> { w: W, buf: u8 }

impl<W> AdapterU8s<W> {
    pub fn new(w: W) -> Self { Self{w, buf: 0} }
}

impl<W: DcxPin> DcxPin for AdapterU8s<W> {
    fn set_dcx_command_mode(&mut self) { self.w.set_dcx_command_mode(); }
    fn set_dcx_data_mode(&mut self) { self.w.set_dcx_data_mode(); }
}

impl<'a, W: Read<'a>> Read<'a> for AdapterU8s<W> {
    type ReadBitsType = <W as Read<'a>>::ReadBitsType;

    fn start_reading(&'a mut self) -> Self::ReadBitsType {
        self.w.start_reading()
    }
}

impl<'a, W: WriteU8s<'a>> WriteU8s<'a> for AdapterU8s<W> {
    type WriteU8sDone = <W as WriteU8s<'a>>::WriteU8sDone;

    fn write_u8s(&'a mut self, data: &'a [u8]) -> Self::WriteU8sDone {
        self.w.write_u8s(data)
    }
}

impl<'a, W: WriteU8s<'a>> WriteU8<'a> for AdapterU8s<W> {
    type WriteU8Done = <W as WriteU8s<'a>>::WriteU8sDone;

    fn write_u8(&'a mut self, data: u8) -> Self::WriteU8Done {
        self.buf = data;
        self.w.write_u8s(core::slice::from_ref(&self.buf))
    }
}

#[cfg(test)]
mod adapter_u8s_tests {
    use predicates::prelude::*;
    use mockall::Sequence;
    use std::{boxed::Box, format, vec::Vec};  // TODO: Remove after mockall 0.9.2+.

    use crate::spi::ReadBits as _;
    use crate::testing_device::{block_on, MockDevice};
    use super::*;

    #[mockall::automock]
    trait BatchIO {
        fn write(&mut self, data: &[u8]);
    }

    #[derive(Default)]
    struct MockBatchDevice { mock: MockBatchIO }

    impl<'a> WriteU8s<'a> for MockBatchDevice {
        type WriteU8sDone = Pin<Box<dyn Future<Output=()> + 'a>>;

        fn write_u8s(&'a mut self, data: &'a [u8]) -> Self::WriteU8sDone {
            Box::pin(async move { self.mock.write(data); })
        }
    }

    fn create_batch_mock() -> AdapterU8s<MockBatchDevice> {
        AdapterU8s::new(Default::default())
    }

    #[test]
    fn write_u8s_as_is() {
        let mut a = create_batch_mock();
        let eq = predicate::function(|array| {
            array == &[0x35, 0x46, 0x12, 0xFF]
        });
        a.w.mock.expect_write()
            .with(eq)
            .times(1);
        block_on(a.write_u8s(&[0x35, 0x46, 0x12, 0xFF]));
    }

    #[test]
    fn write_u8() {
        let mut a = create_batch_mock();
        let eq = predicate::function(|array| { array == &[0x37] });
        a.w.mock.expect_write()
            .with(eq)
            .times(1);
        block_on(a.write_u8(0x37));
    }

    #[test]
    fn dcx_modes() {
        let mut a = AdapterU8::new(MockDevice::new());
        let mut seq = Sequence::new();
        a.w.mock().expect_write_data()
            .with(predicate::eq(9))
            .times(1)
            .in_sequence(&mut seq);
        a.w.mock().expect_write_command()
            .with(predicate::eq(8))
            .times(1)
            .in_sequence(&mut seq);
        a.w.mock().expect_write_data()
            .with(predicate::eq(7))
            .times(1)
            .in_sequence(&mut seq);
        a.w.mock().expect_write_command()
            .with(predicate::eq(6))
            .times(1)
            .in_sequence(&mut seq);
        a.set_dcx_data_mode();
        block_on(a.write_u8(9));
        a.set_dcx_command_mode();
        block_on(a.write_u8(8));
        a.set_dcx_data_mode();
        block_on(a.write_u8(7));
        a.set_dcx_command_mode();
        block_on(a.write_u8(6));
    }

    #[test]
    fn read_as_is() {
        let src: u32 = 0b111010;
        let src_len: usize = 6;

        let mut a = AdapterU8::new(MockDevice::new());
        let mut seq = Sequence::new();
        a.w.mock().expect_start_reading().times(1).in_sequence(&mut seq);
        for i in (0..src_len).rev() {
            let bit = src >> i & 1 != 0;
            a.w.mock().expect_read_bit()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move || bit);
        }
        a.w.mock().expect_finish_reading().times(1).in_sequence(&mut seq);

        let value = block_on(a.start_reading().read_bits(src_len));
        assert_eq!(value, src);
    }
}  // mod adapter_u8s_tests
