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
use std::future::Future;

use super::spi::{DcxPin, Read, ReadModeSetter, WriteU8};

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
    pub fn mock(&mut self) -> &mut MockPlainIO { &mut self.mock }

    pub fn is_data_mode(&self) -> bool { self.is_data_mode }

    pub fn expect_standard_write_command(&mut self,
            seq: &mut mockall::Sequence, command: u8, data: &[u8]) {
        use mockall::predicate::eq;
        self.mock().expect_write_command()
            .with(eq(command))
            .times(1)
            .in_sequence(seq);
        for data in data {
            self.mock().expect_write_data()
                .with(eq(*data))
                .times(1)
                .in_sequence(seq);
        }
    }
}

impl DcxPin for MockDevice {
    fn set_dcx_command_mode(&mut self) { self.is_data_mode = false; }
    fn set_dcx_data_mode(&mut self) { self.is_data_mode = true; }
}

#[async_trait_static::ritit]
impl WriteU8 for MockDevice {
    fn write_u8(&mut self, byte: u8) -> impl Future<Output=()> {
        async move {
            if self.is_data_mode {
                self.mock.write_data(byte);
            } else {
                self.mock.write_command(byte);
            }
        }
    }
}

impl ReadModeSetter for MockDevice {
    fn start_reading(&mut self) { self.mock.start_reading(); }
    fn finish_reading(&mut self) { self.mock.finish_reading(); }
}

#[async_trait_static::ritit]
impl Read for MockDevice {
    fn read(&mut self, num_bits: usize) -> impl Future<Output=u32> {
        async move {
            let mut r: u32 = 0;
            for _ in 0..num_bits {
                r = r.wrapping_shl(1) | (self.mock.read_bit() as u32);
            }
            r
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcU8 {
    Command(u8),
    Data(u8),
}

/// Helper class that records `write_u8()` of [WriteU8] to a vec.
#[derive(Default)]
pub struct FakeDevice {
    is_data_mode: bool,
    seq: std::vec::Vec<DcU8>,
}

impl FakeDevice {
    pub fn new() -> Self { Default::default() }

    /// Gets the recorded sequence.
    pub fn seq(&self) -> std::vec::Vec<DcU8> { self.seq.clone() }
}

impl DcxPin for FakeDevice {
    fn set_dcx_command_mode(&mut self) { self.is_data_mode = false; }
    fn set_dcx_data_mode(&mut self) { self.is_data_mode = true; }
}

#[async_trait_static::ritit]
impl WriteU8 for FakeDevice {
    fn write_u8(&mut self, byte: u8) -> impl Future<Output=()> {
        async move {
            if self.is_data_mode {
                self.seq.push(DcU8::Data(byte));
            } else {
                self.seq.push(DcU8::Command(byte));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Nothing here. This module is joint-tested with [crate::adapters].
}
