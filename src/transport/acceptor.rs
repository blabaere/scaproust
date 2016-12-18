// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio::Ready;

use transport::endpoint::EndpointRegistrar;
use transport::pipe::Pipe;

pub enum Command {
    Open,
    Close
}

pub enum Event {
    Opened,
    Closed,
    Accepted(Vec<Box<Pipe>>),
    Error(io::Error)
}

pub trait Acceptor {
    fn ready(&mut self, ctx: &mut Context, events: Ready);
    fn open(&mut self, ctx: &mut Context);
    fn close(&mut self, ctx: &mut Context);
}

pub trait Context : EndpointRegistrar {
    fn raise(&mut self, evt: Event);
}
