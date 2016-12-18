// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


mod stub;

use std::io;
use std::path;

use transport::{Transport, Destination};
use transport::pipe::Pipe;
use transport::acceptor::Acceptor;
use io_error::*;

pub struct Ipc;

impl Transport for Ipc {
    fn connect(&self, dest: &Destination) -> io::Result<Box<Pipe>> {
        Err(other_io_error("Not implemented !!!"))
    }

    fn bind(&self, dest: &Destination) -> io::Result<Box<Acceptor>> {
        Err(other_io_error("Not implemented !!!"))
    }
}
