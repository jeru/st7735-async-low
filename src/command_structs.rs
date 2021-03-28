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

use paste::paste;

macro_rules! define_pub_bit_type {
    ($name:ident, zero: $zero_value:ident, one: $one_value:ident,
                  doc: $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $zero_value = 0,
            $one_value = 1,
        }
        impl $name {
            fn from_bool(b: bool) -> Self {  // Private.
                if b { Self::$zero_value } else { Self::$one_value }
            }
            fn to_bool(&self) -> bool {  // Private.
                match *self {
                    Self::$zero_value => false,
                    Self::$one_value => true,
                }
            }
        }
        impl Default for $name {
            fn default() -> Self { Self::$zero_value }
        }
        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter)
                    -> ::core::fmt::Result {
                <Self as core::fmt::Debug>::fmt(self, f)
            }
        }
    };
}
macro_rules! bit_field {
    ($name:ident, type: $type:ty, bit_offset: $i:expr) => {
        pub fn $name(&self) -> $type {
            <$type>::from_bool((self.data >> $i) & 1 == 1)
        }
        paste! {
            pub fn [<set_ $name>](&mut self, value: $type) -> &mut Self {
                if value.to_bool() {
                    self.data &= !(1 << $i);
                } else {
                    self.data |= 1 << $i;
                }
                self
            }
        }
    }
}

/// Defines the orientation parameters of the screen.
///
/// # Example
///
/// ```
/// # use st7735_async_low::*;
/// let mut mctl = Madctl::default();
/// mctl.set_row_address_order(RowOrder::TopToBottom)
///     .set_column_address_order(ColumnOrder::LeftToRight)
///     .set_row_column_swap(RowColumnSwap::Swapped)
///     .set_vertical_refresh_order(RowOrder::BottomToTop)
///     .set_horizontal_refresh_order(ColumnOrder::RightToLeft)
///     .set_rgb_order(ColorComponentOrder::BlueGreenRed);
/// assert_eq!(mctl.row_address_order(), RowOrder::TopToBottom);
/// assert_eq!(mctl.row_column_swap(), RowColumnSwap::Swapped);
/// // Can invoke `Commands::madctl(mctl)` to send it to the LCD.
/// ```
#[derive(Clone, Copy, Default)]
pub struct Madctl {
    data: u8,
}
impl Madctl {
    bit_field!(row_address_order, type: RowOrder, bit_offset: 7);
    bit_field!(column_address_order, type: ColumnOrder, bit_offset: 6);
    bit_field!(row_column_swap, type: RowColumnSwap, bit_offset: 5);
    bit_field!(vertical_refresh_order, type: RowOrder, bit_offset: 4);
    bit_field!(horizontal_refresh_order, type: ColumnOrder, bit_offset: 2);
    bit_field!(rgb_order, type: ColorComponentOrder, bit_offset: 3);
}
impl From<Madctl> for u8 {
    fn from(mctl: Madctl) -> u8 { mctl.data }
}

define_pub_bit_type!(RowOrder, zero: TopToBottom, one: BottomToTop,
                     doc: "The row order of the LCD pixels.");

define_pub_bit_type!(ColumnOrder, zero: LeftToRight, one: RightToLeft,
                     doc: "The column order of the LCD pixels.");
define_pub_bit_type!(RowColumnSwap, zero: Unswapped, one: Swapped,
                     doc: "Whether to swap the row and column definitions, \
                     i.e., to switch between the portrait and landscape mode.");
define_pub_bit_type!(ColorComponentOrder, zero: RedGreenBlue, one: BlueGreenRed,
                     doc: "R/G/B component order inside a pixel.");

/// Color mode (the bit widths of the R, G and B components of a pixel).
///
/// The native format is 6-bit for each component. When another (smaller) mode
/// is used, the LCD will internally translate each component into the 6-bit
/// format with a lookup table. See Sec 9.18 "Color Depth Conversion Look Up
/// Tables" of the ST7735S datasheet for the lookup table (LUT).
#[derive(Clone, Copy)]
pub enum Colmod {
    /// Each component has 4 bits. LUT will be used.
    R4G4B4 = 0b011,
    /// Red has 5 bits; green has 6 bits; blue has 5 bits. LUT will be used.
    R5G6B5 = 0b101,
    /// Each compoment has 6 bits. This is the native format; LUT will **not**
    /// be used.
    R6G6B6 = 0b110,
    /// No idea when this value can be used.
    Unknown = 0b111,
}
impl Default for Colmod {
    fn default() -> Self { Self::Unknown }
}
impl From<Colmod> for u8 {
    fn from(colmod: Colmod) -> u8 { colmod as u8 }
}
impl From<u8> for Colmod {
    fn from(raw: u8) -> Self {
        use Colmod::*;
        const R4G4B4_VALUE: u8 = R4G4B4 as u8;
        const R5G6B5_VALUE: u8 = R5G6B5 as u8;
        const R6G6B6_VALUE: u8 = R6G6B6 as u8;
        match raw {
            R4G4B4_VALUE => R4G4B4,
            R5G6B5_VALUE => R5G6B5,
            R6G6B6_VALUE => R6G6B6,
            _ => Unknown,
        }
    }
}
