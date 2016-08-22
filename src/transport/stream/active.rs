// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use mio;

use transport::stream::dead::Dead; 
use transport::stream::{ StepStream, PipeState };
use transport::{ Context, PipeEvt };
use Message;

pub struct Active<T : StepStream + 'static> {
    stream: T
}

impl<T : StepStream> Active<T> {
    pub fn new(stream: T) -> Active<T> {
        Active {
            stream: stream
        }
    }
}

impl<T : StepStream> PipeState<T> for Active<T> {
    fn name(&self) -> &'static str {"Active"}

    fn enter(&self, ctx: &mut Context<PipeEvt>) {
        ctx.raise(PipeEvt::Opened);
    }

    fn open(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        box Dead
    }
    fn close(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        box Dead
    }
    fn send(self: Box<Self>, ctx: &mut Context<PipeEvt>, msg: Rc<Message>) -> Box<PipeState<T>> {
        box Dead
    }
    fn recv(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        box Dead
    }
    fn ready(self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>> {
        box Dead
    }
}
