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
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A simple waker that can be tested whether it is waked.
#[derive(Default)]
pub struct TrivialWaker {
    waked: AtomicBool,
}
impl TrivialWaker {
    pub fn new() -> Self { Default::default() }

    pub fn test_waked_and_clear(&self) -> bool {
        self.waked.swap(false, Ordering::AcqRel)
    }
    fn wake(&self) {
        self.waked.store(true, Ordering::Release);
    }

    pub fn into_raw_waker(&self) -> RawWaker {
        let ptr = self as *const TrivialWaker;
        unsafe { vt_clone(ptr.cast::<()>()) }
    }

    /// Polls and busy-waits until `f` is ready, then returns its result.
    pub fn block_on<F: Future>(&mut self, f: F) -> F::Output {
        let mut f = f;
        let waker = unsafe { Waker::from_raw(self.into_raw_waker()) };
        let mut ctx = Context::from_waker(&waker);

        self.wake();
        loop {
            if !self.test_waked_and_clear() { continue; }
            // Safety: `f` is indeed never moved before it is dropped, which
            // happens at the end of this function.
            let pinned = unsafe { Pin::new_unchecked(&mut f) };
            if let Poll::Ready(v) = pinned.poll(&mut ctx) {
                return v;
            }
        }
    }
}

const TRIVIAL_WAKER_RAW_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(vt_clone, vt_wake, vt_wake, /*drop=*/|_| {});

unsafe fn vt_clone(w: *const ()) -> RawWaker {
    RawWaker::new(w, &TRIVIAL_WAKER_RAW_WAKER_VTABLE)
}

unsafe fn vt_wake(w: *const ()) {
    (*w.cast::<TrivialWaker>()).wake();
}
