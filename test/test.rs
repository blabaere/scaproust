// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#![feature(plugin)]
#![cfg_attr(test, plugin(stainless))]

extern crate scaproust;

mod socket;
mod fair_queue;
mod pipeline;

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

#[cfg(not(windows))]
pub const SYS_TIMEOUT: u64 = 250;

#[cfg(windows)]
pub const SYS_TIMEOUT: u64 = 500;

pub fn sleep_some() {
    thread::sleep(Duration::from_millis(SYS_TIMEOUT));
}

pub fn make_timeout() -> Option<Duration> {
    Some(Duration::from_millis(SYS_TIMEOUT))
}

pub mod urls {
    pub mod tcp {
        // shamelessly copied from mio test module
        use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT};
        use std::sync::atomic::Ordering::SeqCst;

        // Helper for getting a unique port for the task run
        // TODO: Reuse ports to not spam the system
        static mut NEXT_PORT: AtomicUsize = ATOMIC_USIZE_INIT;
        const FIRST_PORT: usize = 18080;

        fn next_port() -> usize {
            unsafe {
                // If the atomic was never used, set it to the initial port
                NEXT_PORT.compare_and_swap(0, FIRST_PORT, SeqCst);

                // Get and increment the port list
                NEXT_PORT.fetch_add(1, SeqCst)
            }
        }

        pub fn get() -> String {
            format!("tcp://127.0.0.1:{}", next_port())
        }
    }

    mod ipc {

    }
}

