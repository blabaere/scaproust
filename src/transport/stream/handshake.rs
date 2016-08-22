// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use mio;

use transport::stream::{ StepStream, PipeState };
use transport::{ Context, PipeEvt };
use Message;

pub struct HandshakeTx<T : StepStream + 'static> {
    stream: T,
    proto_ids: (u16, u16)
}

impl<T : StepStream + 'static> HandshakeTx<T> {
    pub fn new(stream: T, pids: (u16, u16)) -> HandshakeTx<T> {
        HandshakeTx {
            stream: stream,
            proto_ids: pids
        }
    }
}

impl<T : StepStream> PipeState<T> for HandshakeTx<T> {
    fn name(&self) -> &'static str {"HandshakeTx"}
    fn open(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        self
    }
    fn close(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        self
    }
    fn send(self: Box<Self>, ctx: &mut Context<PipeEvt>, msg: Rc<Message>) -> Box<PipeState<T>> {
        self
    }
    fn recv(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        self
    }
    fn ready(self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>> {
        self
    }
}