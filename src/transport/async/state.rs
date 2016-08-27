// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io::{Result, Error};

use mio::EventSet;

use core::Message;
use transport::async::stub::*;
use transport::async::dead::*;
use transport::pipe::{Event, Context};

pub trait PipeState<S : AsyncPipeStub + 'static> {

    fn name(&self) -> &'static str;
    fn open(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        box Dead
    }
    fn close(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        box Dead
    }
    fn send(self: Box<Self>, _: &mut Context, _: Rc<Message>) -> Box<PipeState<S>> {
        box Dead
    }
    fn recv(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        box Dead
    }
    fn error(self: Box<Self>, ctx: &mut Context, err: Error) -> Box<PipeState<S>> {
        ctx.raise(Event::Error(err));

        box Dead
    }
    fn ready(self: Box<Self>, _: &mut Context, _: EventSet) -> Box<PipeState<S>> {
        box Dead
    }
    fn enter(&self, _: &mut Context) {
    }
    fn leave(&self, _: &mut Context) {
    }
}

pub fn transition<F, T, S>(old_state: Box<F>, ctx: &mut Context) -> Box<T> where
    F : PipeState<S>,
    F : Into<T>,
    T : PipeState<S>,
    S : AsyncPipeStub + 'static
{
    old_state.leave(ctx);
    let new_state = Into::into(*old_state);
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
