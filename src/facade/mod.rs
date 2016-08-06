// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::sync::mpsc;
use std::io;

use core;
use ctrl;
use util::*;

use mio;

pub mod session;
pub mod socket;

pub trait Sender<T> {
    fn send(&self, request: T) -> io::Result<()>;
}

pub trait Receiver<T> {
    fn recv(&self) -> io::Result<T>;
}

impl<S, T> Sender<S> for T where 
    T : Deref<Target = mio::Sender<ctrl::EventLoopSignal>>, 
    S : Into<ctrl::EventLoopSignal> {

    fn send(&self, request: S) -> io::Result<()> {
        let signal = request.into();

        self.deref().send(signal).map_err(from_notify_error)
    }

}

impl Into<ctrl::EventLoopSignal> for core::session::Request {
    fn into(self) -> ctrl::EventLoopSignal {
        ctrl::EventLoopSignal::SessionRequest(self)
    }
}

impl Into<ctrl::EventLoopSignal> for core::socket::Request {
    fn into(self) -> ctrl::EventLoopSignal {
        ctrl::EventLoopSignal::SocketRequest(self)
    }
}

impl<T> Receiver<T> for mpsc::Receiver<T> {
    fn recv(&self) -> io::Result<T> {
        match mpsc::Receiver::recv(self) {
            Ok(t)  => Ok(t),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }
}
