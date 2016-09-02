// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod session;
pub mod socket;
pub mod endpoint;
pub mod device;

use std::sync::mpsc;
use std::io;

use mio;

use reactor;
use io_error::*;

pub trait Receiver<T> {
    fn receive(&self) -> io::Result<T>;
}

impl<T> Receiver<T> for mpsc::Receiver<T> {
    fn receive(&self) -> io::Result<T> {
        match mpsc::Receiver::recv(self) {
            Ok(t)  => Ok(t),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }
}

pub type EventLoopRequestSender = mio::channel::Sender<reactor::Request>;