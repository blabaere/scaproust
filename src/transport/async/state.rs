// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io::{Result, Error};

use mio::Ready;

use core::Message;
use transport::async::stub::*;
use transport::async::dead::*;
use transport::pipe::{Event, Context};

pub trait PipeState<S : AsyncPipeStub + 'static> {

    fn name(&self) -> &'static str;
    fn open(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        error!("[{:?}] open while {}", ctx, self.name());
        box Dead
    }
    fn close(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        error!("[{:?}] close while {}", ctx, self.name());
        box Dead
    }
    fn send(self: Box<Self>, ctx: &mut Context, _: Rc<Message>) -> Box<PipeState<S>> {
        error!("[{:?}] send while {}", ctx, self.name());
        box Dead
    }
    fn recv(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        error!("[{:?}] recv while {}", ctx, self.name());
        box Dead
    }
    fn error(self: Box<Self>, ctx: &mut Context, err: Error) -> Box<PipeState<S>> {
        info!("[{:?}] error while {}: {:?}", ctx, self.name(), err);
        
        ctx.raise(Event::Error(err));

        box Dead
    }
    fn ready(self: Box<Self>, ctx: &mut Context, _: Ready) -> Box<PipeState<S>> {
        error!("[{:?}] ready while {}", ctx, self.name());
        box Dead
    }
    fn enter(&mut self, _: &mut Context) {
    }
    fn leave(&mut self, _: &mut Context) {
    }
}

pub fn transition<F, T, S>(mut old_state: Box<F>, ctx: &mut Context) -> Box<T> where
    F : PipeState<S>,
    F : Into<T>,
    T : PipeState<S>,
    S : AsyncPipeStub + 'static
{
    old_state.leave(ctx);
    let mut new_state = Into::into(*old_state);
    new_state.enter(ctx);
    box new_state
}

pub fn transition_if_ok<F, T, S>(f: Box<F>, ctx: &mut Context, res: Result<()>) -> Box<PipeState<S>> where
    F : PipeState<S>,
    F : Into<T>,
    T : PipeState<S> + 'static,
    S : AsyncPipeStub + 'static
{
    match res {
        Ok(..) => transition::<F, T, S>(f, ctx),
        Err(e) => f.error(ctx, e)
    }
}

pub fn no_transition_if_ok<F, S>(f: Box<F>, ctx: &mut Context, res: Result<()>) -> Box<PipeState<S>> where
    F : PipeState<S> + 'static,
    S : AsyncPipeStub + 'static
{
    match res {
        Ok(..) => f,
        Err(e) => f.error(ctx, e)
    }
}
