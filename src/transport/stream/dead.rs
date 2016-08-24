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

pub struct Dead;

impl<T : StepStream> PipeState<T> for Dead {
    fn name(&self) -> &'static str {"Dead"}
    fn enter(&self, ctx: &mut Context<PipeEvt>) {
        ctx.raise(PipeEvt::Closed);
    }
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::cell::RefCell;

    use mio;

    use transport::*;
    use transport::tests::*;
    use transport::stream::*;
    use transport::stream::tests::*;
    use transport::stream::dead::*;
    use Message;

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
            &PipeEvt::Closed => true,
            _ => false,
        };

        assert!(is_closed);
    }
}