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

use crate::spi;
use spi::{WriteBatch, WriteU8};

/// A helper to add [WriteBatch] support when [WriteU8] is implemented.
///
/// Supposedly **not** very efficient. See the Performance Consideration section
/// of [WriteBatch].
///
/// Note that `AdapterU8<st7735_low::testing_device::MockDevice>` provides
/// a mock-based testing utility but is under #[cfg(test)] so isn't rendered
/// on the doc. And `AdapterU8<st7735_low::testing_device::FakeDevice>`
/// simply records the received bytes.
pub struct AdapterU8<W> { w: W }

impl<W> AdapterU8<W> {
    pub fn new(w: W) -> Self { Self{w} }
}

impl<W: spi::DcxPin> spi::DcxPin for AdapterU8<W> {
    fn set_dcx_command_mode(&mut self) { self.w.set_dcx_command_mode(); }
    fn set_dcx_data_mode(&mut self) { self.w.set_dcx_data_mode(); }
}

#[async_trait_static::ritit]
impl<W: spi::Read> spi::Read for AdapterU8<W> {
    #[inline(always)]
    fn read(&mut self, num_bits: usize) -> impl Future<Output=u32> {
        self.w.read(num_bits)
    }
}

impl<W: spi::ReadModeSetter> spi::ReadModeSetter for AdapterU8<W> {
    fn start_reading(&mut self) { self.w.start_reading(); }
    fn finish_reading(&mut self) { self.w.finish_reading(); }
}

#[async_trait_static::ritit]
impl<W: WriteU8> WriteU8 for AdapterU8<W> {
    #[inline(always)]
    fn write_u8(&mut self, data: u8) -> impl Future<Output=()> {
        self.w.write_u8(data)
    }
}

#[async_trait_static::ritit]
impl<W: WriteU8> WriteBatch for AdapterU8<W> {
    #[inline(always)]
    fn write_u8_iter<I: Iterator<Item=u8>>(&mut self, iter: I)
            -> impl Future<Output=()> {
        async move {
            for element in iter {
                self.write_u8(element).await;
            }
        }
    }
    #[inline(always)]
    fn write_u16_iter<I: Iterator<Item=u16>>(&mut self, iter: I)
            -> impl Future<Output=()> {
        async move {
            for element in iter {
                // Big endien.
                self.write_u8((element >> 8) as u8).await;
                self.write_u8((element & 0xFF) as u8).await;
            }
        }
    }
}

#[cfg(test)]
pub mod testing {
    use crate::testing_device::{DcU8, MockPlainIO, MockDevice, FakeDevice};
    impl super::AdapterU8<MockDevice> {
        pub fn new_for_mock() -> Self { Self::new(Default::default()) }

        pub fn mock(&mut self) -> &mut MockPlainIO { self.w.mock() }

        pub fn is_data_mode(&self) -> bool { self.w.is_data_mode() }

        pub fn expect_standard_write_command(&mut self,
                seq: &mut mockall::Sequence, command: u8, data: &[u8]) {
            self.w.expect_standard_write_command(seq, command, data);
        }
    }
    impl super::AdapterU8<FakeDevice> {
        pub fn new_for_fake() -> Self { Self::new(Default::default()) }

        pub fn seq(&self) -> std::vec::Vec<DcU8> { self.w.seq() }
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate;
    use super::*;
    use crate::spi::{DcxPin, WriteU8, write_u8s, write_u16s};
    use crate::testing_device::{block_on, MockDevice};

    fn create_device() -> AdapterU8<MockDevice> {
        AdapterU8::new_for_mock()
    }

    #[test]
    fn test_dcx_pin() {
        let mut device = create_device();
        device.set_dcx_command_mode();
        assert!(!device.is_data_mode());
        device.set_dcx_data_mode();
        assert!(device.is_data_mode());
    }

    #[test]
    fn test_write_u8_command() {
        let mut device = create_device();
        device.mock().expect_write_command()
            .with(predicate::eq(0x35))
            .times(1);
        device.set_dcx_command_mode();
        block_on(device.write_u8(0x35));
    }

    #[test]
    fn test_write_u8_data() {
        let mut device = create_device();
        device.mock().expect_write_data()
            .with(predicate::eq(0x37))
            .times(1);
        device.set_dcx_data_mode();
        block_on(device.write_u8(0x37));
    }

    #[test]
    fn test_write_u8_iter() {
        const COMMAND: u8 = 0x21;
        const DATA: &[u8] = &[0x31, 0x41, 0x51];
        let mut device = create_device();
        let mut seq = mockall::Sequence::new();
        device.mock().expect_write_command()
            .with(predicate::eq(COMMAND))
            .times(1)
            .in_sequence(&mut seq);
        for data in DATA {
            device.mock().expect_write_data()
                .with(predicate::eq(*data))
                .times(1)
                .in_sequence(&mut seq);
        }
        block_on(async {
            device.set_dcx_command_mode();
            device.write_u8(COMMAND).await;
            device.set_dcx_data_mode();
            write_u8s(&mut device, DATA).await;
        });
    }

    #[test]
    fn test_write_u16_iter() {
        const COMMAND: u8 = 0x22;
        const DATA_U16: &[u16] = &[0x3210, 0x6543, 0x9876];  // Big endian.
        const DATA_U8: &[u8] = &[0x32, 0x10, 0x65, 0x43, 0x98, 0x76];
        let mut device = create_device();
        let mut seq = mockall::Sequence::new();
        device.mock().expect_write_command()
            .with(predicate::eq(COMMAND))
            .times(1)
            .in_sequence(&mut seq);
        for data in DATA_U8 {
            device.mock().expect_write_data()
                .with(predicate::eq(*data))
                .times(1)
                .in_sequence(&mut seq);
        }
        block_on(async {
            device.set_dcx_command_mode();
            device.write_u8(COMMAND).await;
            device.set_dcx_data_mode();
            write_u16s(&mut device, DATA_U16).await;
        });
    }

    #[test]
    fn test_write_u8_iter_with_standard_command_expectation() {
        const COMMAND: u8 = 0x21;
        const DATA: &[u8] = &[0x31, 0x41, 0x51];
        let mut device = create_device();
        let mut seq = mockall::Sequence::new();
        device.expect_standard_write_command(&mut seq, COMMAND, DATA);
        block_on(async {
            device.set_dcx_command_mode();
            device.write_u8(COMMAND).await;
            device.set_dcx_data_mode();
            write_u8s(&mut device, DATA).await;
        });
    }

    #[test]
    fn test_fake() {
        let mut device = AdapterU8::new_for_fake();
        block_on(async {
            device.set_dcx_command_mode();
            device.write_u8(0x12).await;
            device.set_dcx_data_mode();
            write_u8s(&mut device, &[0x34]).await;
            device.set_dcx_command_mode();
            device.write_u8(0x56).await;
            device.set_dcx_data_mode();
            write_u16s(&mut device, &[0x789A]).await;
        });
        use crate::testing_device::DcU8::{Command as C, Data as D};
        assert_eq!(device.seq(), std::vec![
            C(0x12), D(0x34), C(0x56), D(0x78), D(0x9A),
        ]);
    }

}  // mod tests
