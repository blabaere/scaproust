// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use core::endpoint::EndpointId;
use Message;

use mio;

// transport must create pipes and acceptors connect, bind and accept results.
// pipes must handle readiness notifications and process send/recv commands
// acceptors must handle readiness notifications and create pipes
// both pipes and acceptors talk to the core via events,
// while the core talks to the transport via commands

// someone in the controller must own the pipes and the acceptors

// transports must be able to provide specialized implementations
// for example tcp and ipc are similar and will work directly on mio streams
// but websocket will differ strongly if it uses a third-party crate.

// Here we can have explicit dependencies on mio (1.0 API preferably).
// As such, any transport is free to implement Evented, and is expected to use EventSet.

pub trait Endpoint<TCmd, TEvt> {
    //fn id(&self) -> EndpointId; // optional ??? id could be stored in the context
    fn ready(&mut self, ctx: &mut Context<TEvt>, events: mio::EventSet);
    fn process(&mut self, ctx: &mut Context<TEvt>, cmd: TCmd);
}

pub trait Context<TEvt> {
    fn register(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()>;
    fn reregister(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()>;
    fn deregister(&mut self, io: &mio::Evented) -> io::Result<()>;

    fn raise(&mut self, evt: TEvt);

    // TODO put logging here ? a log prefix ?
}

pub enum PipeCmd {
    Open,
    Close,
    Send(Message),
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
    Accepted(Vec<Box<Pipe>>),
    Error(io::Error)
}

pub type Pipe = Endpoint<PipeCmd, PipeEvt>;
pub type Acceptor = Endpoint<AcceptorCmd, AcceptorEvt>;

pub trait Transport {
    fn connect(&self) -> io::Result<Box<Pipe>>;

    fn bind(&self) -> io::Result<Box<Acceptor>>;
}
