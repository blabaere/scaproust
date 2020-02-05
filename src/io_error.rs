// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::error;
use std::io;

use mio_extras;

pub fn other_io_error<E>(msg: E) -> io::Error where E: Into<Box<dyn error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::Other, msg)
}

pub fn invalid_data_io_error<E>(msg: E) -> io::Error where E: Into<Box<dyn error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub fn would_block_io_error<E>(msg: E) -> io::Error where E: Into<Box<dyn error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::WouldBlock, msg)
}

pub fn invalid_input_io_error<E>(msg: E) -> io::Error where E: Into<Box<dyn error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

pub fn timedout_io_error<E>(msg: E) -> io::Error where E: Into<Box<dyn error::Error + Send + Sync>> {
    io::Error::new(io::ErrorKind::TimedOut, msg)
}

pub fn from_send_error<T>(send_error: mio_extras::channel::SendError<T>) -> io::Error {
    match send_error {
        mio_extras::channel::SendError::Io(e) => e,
        mio_extras::channel::SendError::Disconnected(_) => other_io_error("channel closed")
    }
}
