// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

/// This module defines various building blocks for transport that uses mio streams.

mod initial;

use std::ops::Deref;
use std::rc::Rc;
use std::io;

use super::*;
use Message;

use mio;

pub trait Sender {
    fn start_send(&mut self, msg: Rc<Message>) -> io::Result<bool>;
    fn resume_send(&mut self) -> io::Result<bool>;
    fn has_pending_send(&self) -> bool;
}

pub trait Receiver {
    fn start_recv(&mut self) -> io::Result<Option<Message>>;
    fn resume_recv(&mut self) -> io::Result<Option<Message>>;
    fn has_pending_recv(&self) -> bool;
}

pub trait Handshake {
    fn send_handshake(&mut self, proto_id: u16) -> io::Result<()>;
    fn recv_handshake(&mut self, proto_id: u16) -> io::Result<()>;
}

pub trait StepStream : Sender + Receiver + Handshake + Deref<Target=mio::Evented> {
}

pub trait PipeState<T : StepStream> {
    fn open(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>>;
    fn close(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>>;
    fn send(self: Box<Self>, ctx: &mut Context<PipeEvt>, msg: Rc<Message>) -> Box<PipeState<T>>;
    fn recv(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>>;

    fn ready(self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>>;
}

pub struct Pipe<T : StepStream + 'static> {
    state: Option<Box<PipeState<T>>>
}

impl<T : StepStream + 'static> Pipe<T> {
    pub fn new(stream: T, proto_id: u16, proto_peer_id: u16) -> Pipe<T> {
        let initial_state = initial::Initial::new(stream, proto_id, proto_peer_id);

        Pipe { state: Some(initial_state) }
    }

    fn apply<F>(&mut self, transition: F) where F : FnOnce(Box<PipeState<T>>) -> Box<PipeState<T>> {
        if let Some(old_state) = self.state.take() {
            let new_state = transition(old_state);

            self.state = Some(new_state);
        }
    }
}

impl<T : StepStream> Endpoint<PipeCmd, PipeEvt> for Pipe<T> {
    fn ready(&mut self, ctx: &mut Context<PipeEvt>, events: mio::EventSet) {
        self.apply(|s| s.ready(ctx, events))
    }
    fn process(&mut self, ctx: &mut Context<PipeEvt>, cmd: PipeCmd) {
        match cmd {
            PipeCmd::Open      => self.apply(|s| s.open(ctx)),
            PipeCmd::Close     => self.apply(|s| s.close(ctx)),
            PipeCmd::Send(msg) => self.apply(|s| s.send(ctx, msg)),
            PipeCmd::Recv      => self.apply(|s| s.recv(ctx))
        }
    }
}