// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use core::endpoint::EndpointId;
use Message;

use mio;

pub mod stream;
pub mod tcp;

pub const DEFAULT_RECV_MAX_SIZE: u64 = 1024 * 1024;

pub trait Endpoint<TCmd, TEvt> {
    //fn id(&self) -> EndpointId; // optional ??? id could be stored in the context
    fn ready(&mut self, ctx: &mut Context<TEvt>, events: mio::EventSet);
    fn process(&mut self, ctx: &mut Context<TEvt>, cmd: TCmd);
}

pub trait Registrar {
    fn register(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()>;
    fn reregister(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()>;
    fn deregister(&mut self, io: &mio::Evented) -> io::Result<()>;
}

pub trait Context<TEvt> : Registrar {

    fn raise(&mut self, evt: TEvt);

}

pub enum PipeCmd {
    Open,
    Close,
    Send(Rc<Message>),
    Recv
}

pub enum PipeEvt {
    Opened,
    Closed,
    Sent,
    Received(Message),
    Error(io::Error)
}

pub enum AcceptorCmd {
    Open,
    Close
}

pub enum AcceptorEvt {
    Opened,
    Closed,
    Accepted(Vec<Box<Endpoint<PipeCmd, PipeEvt>>>),
    Error(io::Error)
}

pub trait Transport {
    fn connect(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Endpoint<PipeCmd, PipeEvt>>>;

    fn bind(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Endpoint<AcceptorCmd, AcceptorEvt>>>;
}
