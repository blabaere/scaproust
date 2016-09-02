// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::error;
use std::io;

use mio;

pub fn other_io_error<E>(msg: E) -> io::Error where E: Into<Box<error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::Other, msg)
}

pub fn invalid_data_io_error<E>(msg: E) -> io::Error where E: Into<Box<error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub fn would_block_io_error<E>(msg: E) -> io::Error where E: Into<Box<error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::WouldBlock, msg)
}

pub fn invalid_input_io_error<E>(msg: E) -> io::Error where E: Into<Box<error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

pub fn timedout_io_error<E>(msg: E) -> io::Error where E: Into<Box<error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::TimedOut, msg)
}

pub fn from_send_error<T>(send_error: mio::channel::SendError<T>) -> io::Error {
    match send_error {
        mio::channel::SendError::Io(e) => e,
        mio::channel::SendError::Disconnected(_) => other_io_error("channel closed")
    }
}

pub fn from_timer_error(timer_error: mio::timer::TimerError) -> io::Error {
    other_io_error(timer_error)
}
