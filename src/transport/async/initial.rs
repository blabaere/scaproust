// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use transport::async::stub::*;
use transport::async::state::*;
use transport::async::handshake::HandshakeTx; 
use transport::async::dead::Dead; 
use transport::pipe::Context;

pub struct Initial<S : AsyncPipeStub> {
    stub: S,
    proto_ids: (u16, u16)
}

impl<S : AsyncPipeStub> Initial<S> {
    pub fn new(s: S, pids: (u16, u16)) -> Initial<S> {
        Initial {
            stub: s,
            proto_ids: pids
        }
    }
}

impl<S : AsyncPipeStub> Into<HandshakeTx<S>> for Initial<S> {
    fn into(self) -> HandshakeTx<S> {
        HandshakeTx::new(self.stub, self.proto_ids)
    }
}

impl<S : AsyncPipeStub + 'static> PipeState<S> for Initial<S> {

    fn name(&self) -> &'static str {"Initial"}
    
    fn open(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        transition::<Initial<S>, HandshakeTx<S>, S>(self, ctx)
    }
    fn close(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        ctx.deregister(self.stub.deref());

        box Dead
    }

}

#[cfg(test)]
mod tests {
    use transport::*;
    use transport::tests::*;
    use transport::stub::*;
    use transport::stub::tests::*;
    use transport::stub::initial::*;

    #[test]
    fn open_should_cause_transition_to_handshake() {
        let stub = TestStepStream::new();
        let state = box Initial::new(stub, (1, 1));
        let mut ctx = TestContext::new();
        let new_state = state.open(&mut ctx);

        assert_eq!(1, ctx.get_registrations().len()); // this is caused by HandshakeTx::enter
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        assert_eq!("HandshakeTx", new_state.name());
    }

    #[test]
    fn close_should_deregister_and_cause_a_transition_to_dead() {
        let stub = TestStepStream::new();
        let state = box Initial::new(stub, (1, 1));
        let mut ctx = TestContext::new();
        let new_state = state.close(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(1, ctx.get_deregistrations());

        assert_eq!("Dead", new_state.name());
    }
}