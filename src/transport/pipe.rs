// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio::EventSet;

use core::Message;
use transport::endpoint::*;

pub enum Command {
    Open,
    Close,
    Send(Rc<Message>),
    Recv
}

pub enum Event {
    Opened,
    Closed,
    CanSend,
    CanRecv,
    Sent,
    Received(Message),
    Error(io::Error)
}

pub trait Pipe {
    fn ready(&mut self, ctx: &mut Context, events: EventSet);
    fn open(&mut self, ctx: &mut Context);
    fn close(&mut self, ctx: &mut Context);
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>);
    fn recv(&mut self, ctx: &mut Context);
}

pub trait Context : EndpointRegistrar {
    fn raise(&mut self, evt: Event);
}