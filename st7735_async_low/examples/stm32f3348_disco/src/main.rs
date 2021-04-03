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

#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate nb;
extern crate panic_halt;
extern crate st7735_async_low;
extern crate stm32f3;

mod spi;
mod trivial_waker;

use core::fmt::{Write as _};
use st7735_async_low::{Colmod, Commands};
use st7735_async_low::adapters::AdapterU8;
use st7735_async_low::spi::WriteU8;


use stm32f3xx_hal as hal;
use hal::delay::Delay;
use hal::prelude::*;
use hal::pac::USART2;
use hal::gpio::{Output, PushPull};
use hal::gpio::{gpioa, gpiob};
use hal::serial::{Serial, Tx};

pub struct Device {
    pub delay: Delay,
    pub csx: gpioa::PA0<Output<PushPull>>,
    pub rst: gpioa::PA1<Output<PushPull>>,
    pub led3: gpiob::PB6<Output<PushPull>>,
    pub tx: TxWrapper,
    pub spi: spi::Spi,
}

// Somehow the trait binding helper of embedded_hal::fmt failed.
pub struct TxWrapper {
    tx: Tx<USART2>,
}
impl core::fmt::Write for TxWrapper {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.as_bytes().into_iter() {
            nb::block!(self.tx.write(*c)).unwrap();
        }
        Ok(())
    }
}

fn setup() -> Device {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = hal::stm32::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();
    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr
        .use_hse(8.mhz())
        .bypass_hse()
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .pclk2(24.mhz())
        .freeze(&mut flash.acr);
    let delay = Delay::new(cp.SYST, clocks);

	let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
	let led3 = gpiob.pb6.into_push_pull_output(
			&mut gpiob.moder, &mut gpiob.otyper);

	let (tx, _rx) = Serial::usart2(
			dp.USART2,
			(gpiob.pb3.into_af7(&mut gpiob.moder, &mut gpiob.afrl),
			 gpiob.pb4.into_af7(&mut gpiob.moder, &mut gpiob.afrl)),
			115200.bps(), clocks, &mut rcc.apb1).split();
    let tx = TxWrapper{tx: tx};

    // ST7735S.
    // LED, CLK, SDA, A0, RST, CS, GND, VCC (pin 1)
	let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut csx = gpioa.pa0.into_push_pull_output(
            &mut gpioa.moder, &mut gpioa.otyper);
    csx.set_high().unwrap();
    let mut rst = gpioa.pa1.into_push_pull_output(
            &mut gpioa.moder, &mut gpioa.otyper);
    rst.set_high().unwrap();
    let sck = gpioa.pa5;
    let sda = gpioa.pa7;
    let dcx = gpioa.pa6.into_push_pull_output(
            &mut gpioa.moder, &mut gpioa.otyper);
    let spi = spi::Spi::new(sck, sda, dcx);

    Device{delay: delay, csx: csx, rst: rst, led3: led3, tx: tx, spi: spi}
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut device = setup();
    let mut csx = device.csx;
    let mut rst = device.rst;
    let mut led3 = device.led3;
    let mut delay = device.delay;
    let mut tx = device.tx;
    writeln!(&mut tx, "Hello.").unwrap();
    delay.delay_ms(300u32);
    writeln!(&mut tx, "{}", device.spi.diagonis()).unwrap();
    let mut cmds = Commands::new(AdapterU8::new(device.spi));
    rst.set_low().unwrap();
    delay.delay_ms(10u32);
    rst.set_high().unwrap();
    csx.set_low().unwrap();
    delay.delay_ms(1u32);
    {
        let mut twaker = trivial_waker::TrivialWaker::new();
        let id1 = twaker.block_on(async { cmds.rdid1().await });
        writeln!(&mut tx, "ID1:{}.", id1).unwrap();
        let id2 = twaker.block_on(async { cmds.rdid2().await });
        writeln!(&mut tx, "ID2:{}.", id2).unwrap();
        let id3 = twaker.block_on(async { cmds.rdid3().await });
        writeln!(&mut tx, "ID3:{}.", id3).unwrap();
        let ids = twaker.block_on(async { cmds.rddid().await });
        writeln!(&mut tx, "ID1:{} ID2:{} ID3:{}.",
                 ids[0], ids[1], ids[2]).unwrap();
    }
    writeln!(&mut tx, "Done IDs.").unwrap();
    let mut twaker = trivial_waker::TrivialWaker::new();
    { 
        twaker.block_on(async {
            cmds.slpout().await;
            cmds.noron().await;
            cmds.dispon().await;
            cmds.colmod(Colmod::R6G6B6).await;
            cmds.raset(0, 126).await;
            cmds.caset(0, 126).await;
        });
    }
    if false {
        loop {
            led3.set_high().unwrap();
            delay.delay_ms(300u32);
            led3.set_low().unwrap();
            delay.delay_ms(300u32);
        }
    } else {
        loop {
            let fill = async {
                let mut w = cmds.ramwr().await;
                for d in 0u32..(1 << 18u32) {
                    let r = (d >> 12) * 4;
                    w.write_u8(r as u8).await;
                    let g = (d >> 6 & 0x3F) * 4;
                    w.write_u8(g as u8).await;
                    let b = (d & 0x3F) * 4;
                    w.write_u8(b as u8).await;
                }
            };
            twaker.block_on(fill);
        }
    }
}
