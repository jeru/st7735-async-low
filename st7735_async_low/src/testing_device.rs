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

use std::{boxed::Box, format, vec::Vec};  // TODO: Remove after mockall 0.9.2+.
use std::pin::Pin;
use std::future::Future;

use crate::spi::{DcxPin, Read, ReadBits, WriteU8, WriteU8s};

pub fn block_on<F: Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    rt.block_on(f)
}

#[mockall::automock]
pub trait PlainIO {
    fn write_command(&mut self, byte: u8);
    fn write_data(&mut self, byte: u8);

    fn start_reading(&mut self);
    fn read_bit(&mut self) -> bool;
    fn finish_reading(&mut self);
}

/// Helper class that delegates `write_u8()` of [WriteU8] to `MockPlainIO`, the
/// `mockall` mocked version of [PlainIO].
#[derive(Default)]
pub struct MockDevice {
    mock: MockPlainIO,
    is_data_mode: bool,
}

impl MockDevice {
    pub fn new() -> Self { Default::default() }
    pub fn mock(&mut self) -> &mut MockPlainIO { &mut self.mock }

    pub fn is_data_mode(&self) -> bool { self.is_data_mode }

    pub fn expect_standard_write_command(&mut self, command: u8, data: &[u8]) {
        let mut seq = mockall::Sequence::new();
        use mockall::predicate::eq;
        self.mock().expect_write_command()
            .with(eq(command))
            .times(1)
            .in_sequence(&mut seq);
        for data in data {
            self.mock().expect_write_data()
                .with(eq(*data))
                .times(1)
                .in_sequence(&mut seq);
        }
    }
}

impl DcxPin for MockDevice {
    fn set_dcx_command_mode(&mut self) { self.is_data_mode = false; }
    fn set_dcx_data_mode(&mut self) { self.is_data_mode = true; }
}

impl<'a> WriteU8<'a> for MockDevice {
    type WriteU8Done = Pin<Box<dyn Future<Output=()> + 'a>>;

    fn write_u8(&'a mut self, data: u8) -> Self::WriteU8Done {
        Box::pin(async move {
            if self.is_data_mode {
                self.mock.write_data(data);
            } else {
                self.mock.write_command(data);
            }
        })
    }
}

impl<'a> WriteU8s<'a> for MockDevice {
    type WriteU8sDone = Pin<Box<dyn Future<Output=()> + 'a>>;

    fn write_u8s(&'a mut self, data: &'a [u8]) -> Self::WriteU8sDone {
        Box::pin(async move {
            for one in data { self.write_u8(*one).await; }
        })
    }
}

impl<'a> Read<'a> for MockDevice {
    type ReadBitsType = MockDeviceReader<'a>;

    fn start_reading(&'a mut self) -> Self::ReadBitsType {
        self.mock.start_reading();
        MockDeviceReader{d: self}
    }
}

pub struct MockDeviceReader<'d> { d: &'d mut MockDevice }

impl<'d> Drop for MockDeviceReader<'d> {
    fn drop(&mut self) {
        self.d.mock.finish_reading();
    }
}

impl<'a, 'd> ReadBits<'a> for MockDeviceReader<'d> {
    type ReadBitsDone = Pin<Box<dyn Future<Output=u32> + 'a>>;

    fn read_bits(&'a mut self, num_bits: usize) -> Self::ReadBitsDone {
        Box::pin(async move {
            let mut r: u32 = 0;
            for _ in 0..num_bits {
                r = r.wrapping_shl(1) | (self.d.mock.read_bit() as u32);
            }
            r
        })
    }
}

#[cfg(test)]
mod tests {
    use mockall::Sequence;
    use mockall::predicate::eq;
    use super::*;

    #[test]
    fn write_command() {
        let mut d: MockDevice = Default::default();
        d.mock().expect_write_command()
            .with(eq(0x15))
            .times(1);
        d.set_dcx_command_mode();
        block_on(d.write_u8(0x15));
    }

    #[test]
    fn write_data_u8() {
        let mut d: MockDevice = Default::default();
        d.mock().expect_write_data()
            .with(eq(0x17))
            .times(1);
        d.set_dcx_data_mode();
        block_on(d.write_u8(0x17));
    }

    #[test]
    fn write_data_seq() {
        let mut d: MockDevice = Default::default();
        let mut seq = Sequence::new();
        let data: [u8; 4] = [0x31, 0x51, 0x41, 0x21];
        for one in &data {
            d.mock().expect_write_data()
                .with(eq(*one))
                .times(1)
                .in_sequence(&mut seq);
        }
        d.set_dcx_data_mode();
        block_on(d.write_u8s(&data));
    }

    #[test]
    fn read_data() {
        let mut d: MockDevice = Default::default();
        let mut seq = Sequence::new();

        let val_a = 0b10010101101;
        let len_a = 11;
        let val_b = 0b010101111;
        let len_b = 9;
        d.mock().expect_start_reading().times(1).in_sequence(&mut seq);
        for i in (0..len_a).rev() {
            d.mock().expect_read_bit()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move || val_a >> i & 1 != 0);
        }
        for i in (0..len_b).rev() {
            d.mock().expect_read_bit()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move || val_b >> i & 1 != 0);
        }
        d.mock().expect_finish_reading().times(1).in_sequence(&mut seq);

        let (a, b) = block_on(async {
            let mut r = d.start_reading();
            let a = r.read_bits(len_a).await;
            let b = r.read_bits(len_b).await;
            (a, b)
        });
        assert_eq!(a, val_a);
        assert_eq!(b, val_b);
    }

}  // mod tests
