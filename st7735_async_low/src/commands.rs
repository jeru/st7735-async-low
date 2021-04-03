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

use super::command_structs::*;
use super::spi::{DcxPin, Read, WriteU8, WriteBatch, write_u16s};

/// Commands of ST7735 in their original form, except that the parameters
/// of each command are typed.
pub struct Commands<S> { spi: S }

impl<S: DcxPin + WriteU8 + WriteBatch> Commands<S> {
    /// Creates a new instance with an spi object.
    pub fn new(mut spi: S) -> Self {
        spi.set_dcx_command_mode();
        Self{spi}
    }

    /// Sets the column address window as `begin` to `end`, both inclusive.
    #[inline(always)]
    pub async fn caset(&mut self, begin: u16, end: u16) {
        self.command_with_u16_pair(0x2A, begin, end).await;
    }

    /// Sets the row address window as `begin` to `end`, both inclusive.
    #[inline(always)]
    pub async fn raset(&mut self, begin: u16, end: u16) {
        self.command_with_u16_pair(0x2B, begin, end).await;
    }

    /// Starts writing memory. The returned object can be used to actually do
    /// the memory writing.
    #[inline(always)]
    pub async fn ramwr(&mut self) -> RamWriter<'_, S> {
        self.command(0x2C).await;
        self.spi.set_dcx_data_mode();
        // `RamWriter::drop()` will restore to command mode.
        RamWriter{spi: &mut self.spi}
    }

    /// Starts writing the RGB lookup table (see the ST7735S datasheet
    /// sec 9.18).
    ///
    /// The returned object can be used to actually do the memory
    /// writing. The user is expected to write exactly 128 bytes, which is
    /// **not** enforced by the library.
    ///
    /// The lookup table is needed when the color mode
    /// (see [colmod()](Self::colmod))
    /// is *not* [Colmod::R6G6B6].
    #[inline(always)]
    pub async fn rgbset(&mut self) -> RamWriter<'_, S> {
        self.command(0x2D).await;
        self.spi.set_dcx_data_mode();
        // `RamWriter::drop()` will restore to command mode.
        RamWriter{spi: &mut self.spi}
    }

    /// Sets the partial area address window as `begin` to `end`, both
    /// inclusive.
    #[inline(always)]
    pub async fn ptlar(&mut self, begin: u16, end: u16) {
        self.command_with_u16_pair(0x30, begin, end).await;
    }

    /// Sets the scroll area address windows.
    #[inline(always)]
    pub async fn scrlar(&mut self, top: u16, visible: u16, bottom: u16) {
        self.command_with_u16_slice(0x33, &[top, visible, bottom]).await;
    }

    // Performance-critical enough to have its instantiated version.
    async fn command_with_u16_pair(
            &mut self, cmd: u8, first: u16, second: u16) {
        self.command(cmd).await;
        self.spi.set_dcx_data_mode();
        write_u16s(&mut self.spi, &[first, second]).await;
        self.spi.set_dcx_command_mode();
    }

    async fn command_with_u16_slice(
            &mut self, cmd: u8, data: &[u16]) {
        self.command(cmd).await;
        self.spi.set_dcx_data_mode();
        write_u16s(&mut self.spi, data).await;
        self.spi.set_dcx_command_mode();
    }
}

impl<S: DcxPin + WriteU8> Commands<S> {
    #[inline(always)]
    async fn command(&mut self, cmd: u8) {
        self.spi.write_u8(cmd).await;
    }

    async fn command_with_u8(&mut self, cmd: u8, data: u8) {
        self.command(cmd).await;
        self.spi.set_dcx_data_mode();
        self.spi.write_u8(data).await;
        self.spi.set_dcx_command_mode();
    }

    /// Does nothing.
    #[inline(always)]
    pub async fn nop(&mut self) { self.command(0x00).await; }
    /// Software-resets.
    #[inline(always)]
    pub async fn swreset(&mut self) { self.command(0x01).await; }
    /// Enters the sleep mode.
    #[inline(always)]
    pub async fn slpin(&mut self) { self.command(0x10).await; }
    /// Exits the sleep mode.
    #[inline(always)]
    pub async fn slpout(&mut self) { self.command(0x11).await; }
    /// Enters the partial mode.
    #[inline(always)]
    pub async fn ptlon(&mut self) { self.command(0x12).await; }
    /// Enters the normal mode (i.e., exits the partial mode).
    #[inline(always)]
    pub async fn noron(&mut self) { self.command(0x13).await; }
    /// Disables the inversion mode.
    #[inline(always)]
    pub async fn invoff(&mut self) { self.command(0x20).await; }
    /// Enables the inversion mode.
    #[inline(always)]
    pub async fn invon(&mut self) { self.command(0x21).await; }
    // GAMSET skipped.
    /// Turns the display/screen off.
    #[inline(always)]
    pub async fn dispoff(&mut self) { self.command(0x28).await; }
    /// Turns the display/screen on.
    #[inline(always)]
    pub async fn dispon(&mut self) { self.command(0x29).await; }
    /// Turns the tear effect line off.
    #[inline(always)]
    pub async fn teoff(&mut self) { self.command(0x34).await; }
    /// Turns the tear effect line on with the given mode.
    #[inline(always)]
    pub async fn teon(&mut self, te_mode: bool) {
        self.command_with_u8(0x35, if te_mode {1} else {0}).await; }
    /// Sets the MADCTL register.
    #[inline(always)]
    pub async fn madctl(&mut self, data: Madctl) {
        self.command_with_u8(0x36, data.into()).await; }
    // VSCSAD skipped.
    /// Turns the idle mode off, i.e., enables the full color mode.
    #[inline(always)]
    pub async fn idmoff(&mut self) { self.command(0x38).await; }
    /// Turns the idle mode on, i.e., enables the 8-color mode.
    #[inline(always)]
    pub async fn idmon(&mut self) { self.command(0x39).await; }
    /// Sets the color mode, i.e., how many bits of the R, G and B components
    /// have.
    #[inline(always)]
    pub async fn colmod(&mut self, data: Colmod) {
        self.command_with_u8(0x3A, data.into()).await; }

    // Panel functions skipped.
}

/// A helper RAII object that can write data in u8 or u16 forms. It keeps
/// borrowing. Dropping it makes the command that creates this instance
/// end.
pub struct RamWriter<'a, S: DcxPin> { spi: &'a mut S }

impl<'a, S: DcxPin> Drop for RamWriter<'a, S> {
    #[inline(always)]
    fn drop(&mut self) { self.spi.set_dcx_command_mode(); }
}

#[async_trait_static::ritit]
impl<'a, S: DcxPin + WriteU8> WriteU8 for RamWriter<'a, S> {
    #[inline(always)]
    fn write_u8(&mut self, data: u8) -> impl Future<Output=()> {
        self.spi.write_u8(data)
    }
}

#[async_trait_static::ritit]
impl<'a, S: DcxPin + WriteBatch> WriteBatch for RamWriter<'a, S> {
    #[inline(always)]
    fn write_u8_iter<I: Iterator<Item=u8>>(&mut self, iter: I)
            -> impl Future<Output=()> {
        self.spi.write_u8_iter(iter)
    }
    fn write_u16_iter<I: Iterator<Item=u16>>(&mut self, iter: I)
            -> impl Future<Output=()> {
        self.spi.write_u16_iter(iter)
    }
}

impl<S: DcxPin + WriteU8 + Read> Commands<S> {
    #[inline(always)]
    async fn read_command(&mut self, cmd: u8, num_bits: usize) -> u32 {
        self.command(cmd).await;
        self.spi.start_reading();
        let r = self.spi.read(num_bits).await;
        self.spi.finish_reading();
        r
    }

    // RD* (except RDDID and RDID*) skipped.
    // RAMRD skipped.

    /// Reads `ID1`, `ID2` and `ID3` of the screen with a single command.
    #[inline(always)]
    pub async fn rddid(&mut self) -> [u8; 3] {
        let r = self.read_command(0x04, 25).await;
        [(r >> 16) as u8, (r >> 8 & 0xFF) as u8, (r & 0xFF) as u8]
    }

    /// Reads `ID1`, i.e., the manufacturer ID. Unless reprogrammed, the value
    /// should be 0x7C (decimal 124).
    #[inline(always)]
    pub async fn rdid1(&mut self) -> u8 {
        self.read_command(0xDA, 8).await as u8
    }

    /// Reads `ID2`' i.e., the LCD's "module/driver version ID". The highest
    /// bit is always 1.
    #[inline(always)]
    pub async fn rdid2(&mut self) -> u8 {
        self.read_command(0xDB, 8).await as u8
    }

    /// Reads `ID3`, i.e., the LCD's "module/driver ID".
    #[inline(always)]
    pub async fn rdid3(&mut self) -> u8 {
        self.read_command(0xDC, 8).await as u8
    }
}

#[cfg(test)]
mod tests {
    use std::vec;
    use std::vec::Vec;
    use crate::AdapterU8;
    use crate::spi::{write_u8s, write_u16s, WriteU8};
    use crate::testing_device::{
        block_on, DcU8, FakeDevice, MockDevice, MockPlainIO};
    use mockall::{predicate, Sequence};
    use super::Commands;

    impl Commands<AdapterU8<FakeDevice>> {
        pub fn seq(&self) -> Vec<DcU8> { self.spi.seq() }
    }

    fn create_fake() -> Commands<AdapterU8<FakeDevice>> {
        Commands::new(AdapterU8::new_for_fake())
    }

    macro_rules! test_simple_write {
        ($fn:tt $args:tt, code: $code:expr, data: $data:expr) => {
            #[test]
            fn $fn() {
                let mut cmds = create_fake();
                block_on(cmds.$fn$args);
                let mut expected = vec![DcU8::Command($code)];
                expected.extend($data.iter().map(|b| DcU8::Data(*b)));
                assert_eq!(cmds.seq(), expected);
            }
        };
    }

    test_simple_write!(nop(), code: 0x00, data: &[]);
    test_simple_write!(swreset(), code: 0x01, data: &[]);
    test_simple_write!(slpin(), code: 0x10, data: &[]);
    test_simple_write!(slpout(), code: 0x11, data: &[]);
    test_simple_write!(ptlon(), code: 0x12, data: &[]);
    test_simple_write!(noron(), code: 0x13, data: &[]);
    test_simple_write!(invoff(), code: 0x20, data: &[]);
    test_simple_write!(invon(), code: 0x21, data: &[]);
    // GAMSET (26h) skipped.
    test_simple_write!(dispoff(), code: 0x28, data: &[]);
    test_simple_write!(dispon(), code: 0x29, data: &[]);
    test_simple_write!(caset(0x1234, 0x5678), code: 0x2A,
                       data: &[0x12, 0x34, 0x56, 0x78]);
    test_simple_write!(raset(0x9876, 0x5432), code: 0x2B,
                       data: &[0x98, 0x76, 0x54, 0x32]);
    #[test]
    fn ramwr() {
        let mut cmds = create_fake();
        block_on(async {
            let mut rw = cmds.ramwr().await;
            rw.write_u8(0x01).await;
            write_u8s(&mut rw, &[0x23, 0x45]).await;
            write_u8s(&mut rw, &[]).await;
            write_u16s(&mut rw, &[0x6789, 0xABCD]).await;
            write_u16s(&mut rw, &[]).await;
        });
        use DcU8::Command as C;
        use DcU8::Data as D;
        assert_eq!(cmds.seq(), vec![
            C(0x2C), D(0x01), D(0x23), D(0x45), D(0x67), D(0x89), D(0xAB),
            D(0xCD),
        ]);
    }
    #[test]
    fn rgbset() {
        let mut cmds = create_fake();
        let mut expected = std::vec![DcU8::Command(0x2D)];
        expected.extend(&[DcU8::Data(0x35); 128]);
        block_on(async {
            let mut rw = cmds.rgbset().await;
            rw.write_u8(0x35).await;
            write_u8s(&mut rw, &[0x35; 27]).await;
            write_u16s(&mut rw, &[0x3535; 50]).await;
        });
        assert_eq!(cmds.seq(), expected);
    }
    test_simple_write!(ptlar(0x1357, 0x2468), code: 0x30,
                       data: &[0x13, 0x57, 0x24, 0x68]);
    test_simple_write!(scrlar(0x2143, 0x3254, 0x4365), code: 0x33,
                       data: &[0x21, 0x43, 0x32, 0x54, 0x43, 0x65]);
    test_simple_write!(teoff(), code: 0x34, data: &[]);
    #[test]
    fn teon_mode0() {
        let mut cmds = create_fake();
        block_on(cmds.teon(false));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x35), DcU8::Data(0x00)]);
    }
    #[test]
    fn teon_mode1() {
        let mut cmds = create_fake();
        block_on(cmds.teon(true));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x35), DcU8::Data(0x01)]);
    }
    #[test]
    fn madctl_test0() {
        use crate::command_structs::{
            Madctl, RowOrder, ColumnOrder, RowColumnSwap, ColorComponentOrder};
        let mut mctl = Madctl::default();
        mctl.set_row_address_order(RowOrder::TopToBottom)
            .set_column_address_order(ColumnOrder::LeftToRight)
            .set_row_column_swap(RowColumnSwap::Swapped)
            .set_vertical_refresh_order(RowOrder::BottomToTop)
            .set_horizontal_refresh_order(ColumnOrder::RightToLeft)
            .set_rgb_order(ColorComponentOrder::BlueGreenRed);

        let mut cmds = create_fake();
        block_on(cmds.madctl(mctl));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x36), DcU8::Data(0xC0)]);
    }
    #[test]
    fn madctl_test1() {
        use crate::command_structs::{
            Madctl, RowOrder, ColumnOrder, RowColumnSwap, ColorComponentOrder};
        let mut mctl = Madctl::default();
        mctl.set_row_address_order(RowOrder::BottomToTop)
            .set_column_address_order(ColumnOrder::RightToLeft)
            .set_row_column_swap(RowColumnSwap::Unswapped)
            .set_vertical_refresh_order(RowOrder::TopToBottom)
            .set_horizontal_refresh_order(ColumnOrder::LeftToRight)
            .set_rgb_order(ColorComponentOrder::RedGreenBlue);

        let mut cmds = create_fake();
        block_on(cmds.madctl(mctl));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x36), DcU8::Data(0x3C)]);
    }
    // VSCSAD skipped.
    test_simple_write!(idmoff(), code: 0x38, data: &[]);
    test_simple_write!(idmon(), code: 0x39, data: &[]);
    #[test]
    fn colmod_r4g4b4() {
        use crate::command_structs::Colmod;
        let mut cmds = create_fake();
        block_on(cmds.colmod(Colmod::R4G4B4));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x3A), DcU8::Data(0b011)]);
    }
    #[test]
    fn colmod_r5g6b5() {
        use crate::command_structs::Colmod;
        let mut cmds = create_fake();
        block_on(cmds.colmod(Colmod::R5G6B5));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x3A), DcU8::Data(0b101)]);
    }
    #[test]
    fn colmod_r6g6b6() {
        use crate::command_structs::Colmod;
        let mut cmds = create_fake();
        block_on(cmds.colmod(Colmod::R6G6B6));
        assert_eq!(cmds.seq(), vec![DcU8::Command(0x3A), DcU8::Data(0b110)]);
    }

    // Panel functions skipped.

    impl Commands<AdapterU8<MockDevice>> {
        fn mock(&mut self) -> &mut MockPlainIO {
            self.spi.mock()
        }
    }

    fn create_mock() -> Commands<AdapterU8<MockDevice>> {
        Commands::new(AdapterU8::new_for_mock())
    }

    fn set_read_command_expectations(
            mock: &mut MockPlainIO, code: u8, bits: &str) {
        let mut seq = Sequence::new();
        mock.expect_write_command()
            .with(predicate::eq(code))
            .times(1)
            .in_sequence(&mut seq);
        mock.expect_start_reading()
            .times(1)
            .in_sequence(&mut seq);
        for c in bits.chars() {
            mock.expect_read_bit()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move || c != '0');
        }
        mock.expect_finish_reading()
            .times(1)
            .in_sequence(&mut seq);
    }

    #[test]
    fn rdid1() {
        let mut cmds = create_mock();
        const DATA: u8 = 0b10100110;
        set_read_command_expectations(
                cmds.mock(), 0xDA, &std::format!("{:08b}", DATA));
        let v = block_on(cmds.rdid1());
        assert_eq!(v, DATA);
    }

    #[test]
    fn rdid2() {
        let mut cmds = create_mock();
        const DATA: u8 = 0b01010111;
        set_read_command_expectations(
                cmds.mock(), 0xDB, &std::format!("{:08b}", DATA));
        let v = block_on(cmds.rdid2());
        assert_eq!(v, DATA);
    }

    #[test]
    fn rdid3() {
        let mut cmds = create_mock();
        const DATA: u8 = 0b01100111;
        set_read_command_expectations(
                cmds.mock(), 0xDC, &std::format!("{:08b}", DATA));
        let v = block_on(cmds.rdid3());
        assert_eq!(v, DATA);
    }

    #[test]
    fn rddid() {
        let mut cmds = create_mock();
        const DATA_U32: u32 = 0b0_11110000_11010010_01100001;
        const DATA_ARR: [u8; 3] = [0b11110000, 0b11010010, 0b01100001];
        set_read_command_expectations(
                cmds.mock(), 0x04, &std::format!("{:25b}", DATA_U32));
        let v = block_on(cmds.rddid());
        assert_eq!(v, DATA_ARR);
    }

}  // mod tests
