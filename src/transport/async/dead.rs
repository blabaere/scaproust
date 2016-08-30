// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use mio::Ready;

use core::Message;
use transport::async::stub::*;
use transport::async::state::*;
use transport::pipe::{Event, Context};

pub struct Dead;

impl<S : AsyncPipeStub + 'static> PipeState<S> for Dead {

    fn name(&self) -> &'static str {"Dead"}
    fn enter(&self, ctx: &mut Context) {
        ctx.raise(Event::Closed);
    }
    fn open(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        self
    }
    fn close(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        self
    }
    fn send(self: Box<Self>, _: &mut Context, _: Rc<Message>) -> Box<PipeState<S>> {
        self
    }
    fn recv(self: Box<Self>, _: &mut Context) -> Box<PipeState<S>> {
        self
    }
    fn ready(self: Box<Self>, _: &mut Context, _: Ready) -> Box<PipeState<S>> {
        self
    }

}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::cell::RefCell;

    use mio;

    use transport::*;
    use transport::tests::*;
    use transport::async::state::*;
    use transport::async::tests::*;
    use transport::async::dead::*;

    #[test]
    fn on_enter_an_event_is_raised() {
        let state = box Dead as Box<PipeState<TestStepStream>>;
        let mut ctx = TestPipeContext::new();

        state.enter(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        assert_eq!(1, ctx.get_raised_events().len());
        let ref evt = ctx.get_raised_events()[0];
        let is_closed = match evt {
            &pipe::Event::Closed => true,
            _ => false,
        };

        assert!(is_closed);
    }
}