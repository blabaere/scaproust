// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

/// This module provides a pipe implementation built upon mio async streams.

pub mod stub;
mod state;
mod initial;
mod handshake;
mod active;
mod dead;

#[cfg(test)]
mod tests;

use std::rc::Rc;

use mio;

use core::Message;
use transport::*;
use transport::pipe::*;

use self::stub::AsyncPipeStub;
use self::state::PipeState;

pub struct AsyncPipe<S : AsyncPipeStub + 'static> {

    state: Option<Box<PipeState<S>>>

}

impl<S : AsyncPipeStub + 'static> AsyncPipe<S> {
    pub fn new(stub: S, pids: (u16, u16)) -> AsyncPipe<S> {
        let initial_state = box initial::Initial::new(stub, pids);

        AsyncPipe { state: Some(initial_state) }
    }

    fn apply<F>(&mut self, ctx: &mut Context, transition: F) where F : FnOnce(Box<PipeState<S>>, &mut Context) -> Box<PipeState<S>> {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let new_state = transition(old_state, ctx);
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }
}

impl<S : AsyncPipeStub> pipe::Pipe for AsyncPipe<S> {

    fn ready(&mut self, ctx: &mut Context, events: mio::Ready) {
        self.apply(ctx, |s, ctx| s.ready(ctx, events))
    }

    fn open(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx| s.open(ctx))
    }

    fn close(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx| s.close(ctx))
    }

    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) {
        self.apply(ctx, |s, ctx| s.send(ctx, msg))
    }

    fn recv(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx| s.recv(ctx))
    }
}
