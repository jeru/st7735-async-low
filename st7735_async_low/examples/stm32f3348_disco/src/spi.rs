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
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::{free as interrupt_free};

use super::hal;
use super::hal::prelude::*;
use hal::gpio::{Input, Floating, Output, PushPull};
use hal::gpio::gpioa;
use hal::stm32;

pub struct Spi {
    _sck: gpioa::PA5<Input<Floating>>,
    _sda: gpioa::PA7<Input<Floating>>,
    dcx: gpioa::PA6<Output<PushPull>>,
}
impl Spi {
    pub fn new(sck: gpioa::PA5<Input<Floating>>,
               sda: gpioa::PA7<Input<Floating>>,
               dcx: gpioa::PA6<Output<PushPull>>) -> Self {
        unsafe { initialize_spi1() };
        Self{_sck: sck, _sda: sda, dcx: dcx}
    }

    /// The returned object will, when being dropped, block until the byte
    /// sending is finished.
    fn write_byte(&mut self, byte: u8) -> ByteWriting<'_> {
        unsafe { send_spi1_byte(byte) };
        ByteWriting{status: ByteWritingStatus::Started,
                    lifetime: Default::default()}
    }

    pub fn diagonis(&mut self) -> &'static str {
        let sr = unsafe { spi1_regs().sr.read() };
        if sr.fre().is_error() {
            &"frame format error"
        } else if sr.ovr().is_overrun() {
            &"overrun"
        } else if sr.modf().is_fault() {
            &"mode fault"
        } else if sr.crcerr().is_no_match() {
            &"crc error"
        } else if sr.bsy().is_busy() {
            &"busy"
        } else if sr.txe().is_not_empty() {
            &"txe not empty"
        } else {
            &"txe empty"
        }
    }
}

impl st7735_async_low::spi::DcxPin for Spi {
    fn set_dcx_command_mode(&mut self) { self.dcx.set_low().unwrap(); }
    fn set_dcx_data_mode(&mut self) { self.dcx.set_high().unwrap(); }
}

impl<'a> st7735_async_low::spi::WriteU8<'a> for Spi {
    type WriteU8Done = ByteWriting<'a>;

    fn write_u8(&'a mut self, data: u8) -> Self::WriteU8Done {
        self.write_byte(data)
    }
}

impl<'a> st7735_async_low::spi::Read<'a> for Spi {
    type ReadBitsType = BitsReader<'a>;

    fn start_reading(&'a mut self) -> Self::ReadBitsType {
        unsafe {
            disable_spi1();
            set_pins_bitbang();
        }
        BitsReader{spi: self}
    }
}

pub struct BitsReader<'r> { spi: &'r mut Spi }

impl<'r> Drop for BitsReader<'r> {
    fn drop(&mut self) {
        unsafe {
            set_pins_spi1();
            enable_spi1();
        }
    }
}

impl<'a, 'r> st7735_async_low::spi::ReadBits<'a> for BitsReader<'r> {
    type ReadBitsDone = BitsReaderResult<'a>;

    fn read_bits(&'a mut self, num_bits: usize) -> Self::ReadBitsDone {
        BitsReaderResult{_spi: &mut self.spi, num_bits}
    }
}

pub struct BitsReaderResult<'a> { _spi: &'a mut Spi, num_bits: usize }

impl<'a> Future for BitsReaderResult<'a> {
    type Output = u32;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
            let mut r: u32 = 0;
            let regs = unsafe{ pa_regs() };
            for _ in 0..self.num_bits {
                regs.bsrr.write(|w| w.br5().reset());
                delay();
                let bit = if regs.idr.read().idr7().bits() {1} else {0};
                regs.bsrr.write(|w| w.bs5().set());
                delay();
                r = r.wrapping_shl(1) | bit;
            }
            Poll::Ready(r)
    }
}

fn delay() {
    for _ in 0..10u8 { cortex_m::asm::nop(); }
}

#[derive(Copy, Clone)]
enum ByteWritingStatus {
    Started,
    Done,
}

pub struct ByteWriting<'a> {
    status: ByteWritingStatus,
    lifetime: PhantomData<&'a u8>,
}
impl<'a> ByteWriting<'a> {
    pub fn is_done(&mut self) -> bool {
        let current_status = self.status;
        match current_status {
            ByteWritingStatus::Started => {
                if unsafe { spi1_regs().sr.read().bsy().is_not_busy() } {
                    self.status = ByteWritingStatus::Done;
                    return true;
                }
                return false;
            },
            ByteWritingStatus::Done => {
                return true;
            },
        }
    }
}
impl<'a> Drop for ByteWriting<'a> {
    fn drop(&mut self) {
        while !self.is_done() {}
    }
}

impl<'a> Future for ByteWriting<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if unsafe{self.get_unchecked_mut()}.is_done() {
            return Poll::Ready(());
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// Should be called only once.
/// Safety: assumes the ownership of PA5 and PA7.
unsafe fn initialize_spi1() {
    interrupt_free(|_cs| {
        (&*stm32::RCC::ptr()).apb2enr.modify(|_, w| w.spi1en().enabled());
        pa_regs().afrl.modify(|_, w| w.afrl5().af5()
                                      .afrl7().af5());
        pa_regs().pupdr.modify(|_, w| w.pupdr5().floating()
                                       .pupdr7().floating());
        pa_regs().otyper.modify(|_, w| w.ot5().push_pull());
    });
    set_pins_spi1();
    let spi = spi1_regs();
    // Reference manual dm00093941 29.4.7.
    spi.cr1.modify(|_, w| w
        // Disable the SPI for now.
        .spe().disabled()
        // 2(a)
        .br().div32()
        // 2(b)
        .cpol().idle_high()
        .cpha().second_edge()
        // 2(c) Transmit-only.
        .rxonly().full_duplex()
        .bidimode().bidirectional()
        .bidioe().output_enabled()
        // 2(d) MSB first.
        .lsbfirst().msbfirst()
        // 2(e) No CRC.
        .crcen().disabled()
        // 2(f) No physical NSS pin.
        .ssm().enabled()
        .ssi().slave_not_selected()
        // 2(g) As master.
        .mstr().master()
    );
    spi.cr2.modify(|_, w| w
        // 3(a) Data length.
        .ds().eight_bit()
        // 3(b), (c), (d), (e) Irrelevent.
        // 3(f) LDMA_TX/_RX. Not yet needed.
    );
    // 4 CRC polynomial irrelevant.
    // 5 DMA not yet needed.
    enable_spi1();
}

#[inline(always)]
unsafe fn enable_spi1() {
    spi1_regs().cr1.modify(|_, w| w.spe().enabled());
}
#[inline(always)]
unsafe fn disable_spi1() {
    spi1_regs().cr1.modify(|_, w| w.spe().disabled());
}

#[inline(always)]
unsafe fn send_spi1_byte(byte: u8) {
    let ptr = (&spi1_regs().dr) as *const _ as *mut u8;
    core::ptr::write_volatile(ptr, byte);
}

#[inline(always)]
unsafe fn spi1_regs() -> &'static stm32::spi1::RegisterBlock {
    &*stm32::SPI1::ptr()
}

unsafe fn set_pins_spi1() {
    interrupt_free(|_cs| {
        pa_regs().moder.modify(|_, w| w.moder5().alternate()
                                       .moder7().alternate());
    });
}

unsafe fn set_pins_bitbang() {
    interrupt_free(|_cs| {
        pa_regs().moder.modify(|_, w| w.moder5().output()
                                       .moder7().input());
    });
}

#[inline(always)]
unsafe fn pa_regs() -> &'static stm32::gpioa::RegisterBlock {
    &*stm32::GPIOA::ptr()
}
