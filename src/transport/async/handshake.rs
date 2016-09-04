// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io::Result;

use mio::{Ready, PollOpt};

use transport::async::stub::*;
use transport::async::state::*;
use transport::async::active::Active; 
use transport::async::dead::Dead; 
use transport::pipe::Context;

pub struct HandshakeTx<S : AsyncPipeStub + 'static> {
    stub: S,
    proto_ids: (u16, u16)
}

impl<S : AsyncPipeStub> HandshakeTx<S> {
    pub fn new(s: S, pids: (u16, u16)) -> HandshakeTx<S> {
        HandshakeTx { 
            stub: s,
            proto_ids: pids
        }
    }

    fn send_handshake(&mut self) -> Result<()> {
        let pids = self.proto_ids;

        self.stub.send_handshake(pids)
    }
}

impl<S : AsyncPipeStub> Into<HandshakeRx<S>> for HandshakeTx<S> {
    fn into(self) -> HandshakeRx<S> {
        HandshakeRx::new(self.stub, self.proto_ids)
    }
}

impl<S : AsyncPipeStub> PipeState<S> for HandshakeTx<S> {
    fn name(&self) -> &'static str {"HandshakeTx"}

    fn enter(&self, ctx: &mut Context) {
        ctx.register(self.stub.deref(), Ready::writable(), PollOpt::level());
    }
    fn close(mut self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        self.stub.shutdown();
        ctx.deregister(self.stub.deref());

        box Dead
    }
    fn ready(mut self: Box<Self>, ctx: &mut Context, events: Ready) -> Box<PipeState<S>> {
        if events.is_writable() {
            let res = self.send_handshake();

            transition_if_ok::<HandshakeTx<S>, HandshakeRx<S>, S>(self, ctx, res)
        } else {
            self
        }
    }
}

pub struct HandshakeRx<S> {
    stub: S,
    proto_ids: (u16, u16)
}

impl<S: AsyncPipeStub> HandshakeRx<S> {
    pub fn new(s: S, pids: (u16, u16)) -> HandshakeRx<S> {
        HandshakeRx {
            stub: s,
            proto_ids: pids
        }
    }

    fn recv_handshake(&mut self) -> Result<()> {
        let pids = self.proto_ids;

        self.stub.recv_handshake(pids)
    }
}

impl<S : AsyncPipeStub> Into<Active<S>> for HandshakeRx<S> {
    fn into(self) -> Active<S> {
        Active::new(self.stub)
    }
}

impl<S : AsyncPipeStub + 'static> PipeState<S> for HandshakeRx<S> {

    fn name(&self) -> &'static str {"HandshakeRx"}

    fn enter(&self, ctx: &mut Context) {
        ctx.reregister(self.stub.deref(), Ready::readable(), PollOpt::level());
    }
    fn close(mut self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        self.stub.shutdown();
        ctx.deregister(self.stub.deref());

        box Dead
    }
    fn ready(mut self: Box<Self>, ctx: &mut Context, events: Ready) -> Box<PipeState<S>> {
        if events.is_readable() {
            let res = self.recv_handshake();
            
            transition_if_ok::<HandshakeRx<S>, Active<S>, S>(self, ctx, res)
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::cell::RefCell;

    use mio;

    use transport::tests::*;
    use transport::async::state::*;
    use transport::async::tests::*;
    use transport::async::handshake::*;

    #[test]
    fn on_enter_tx_should_register() {
        let stub = TestStepStream::new();
        let state = box HandshakeTx::new(stub, (4, 2));
        let mut ctx = TestPipeContext::new();

        state.enter(&mut ctx);

        assert_eq!(1, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        let (ref interest, ref poll_opt) = ctx.get_registrations()[0];
        let all = mio::Ready::writable();
        let edge = mio::PollOpt::level();

        assert_eq!(&all, interest);
        assert_eq!(&edge, poll_opt);
    }

    #[test]
    fn tx_close_should_deregister_and_cause_a_transition_to_dead() {
        let stub = TestStepStream::new();
        let state = box HandshakeTx::new(stub, (1, 1));
        let mut ctx = TestPipeContext::new();
        let new_state = state.close(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(1, ctx.get_deregistrations());

        assert_eq!("Dead", new_state.name());
    }

    #[test]
    fn on_writable_the_handshake_should_be_sent() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let pids = (4, 2);
        let state = box HandshakeTx::new(stub, pids);
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::writable();
        let new_state = state.ready(&mut ctx, events);

        assert_eq!(1, sensor.borrow().get_sent_handshakes().len());
        assert_eq!(pids, sensor.borrow().get_sent_handshakes()[0]);

        assert_eq!("HandshakeRx", new_state.name());
    }

    #[test]
    fn on_enter_rx_should_reregister() {
        let stub = TestStepStream::new();
        let state = box HandshakeRx::new(stub, (4, 2));
        let mut ctx = TestPipeContext::new();

        state.enter(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(1, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        let (ref interest, ref poll_opt) = ctx.get_reregistrations()[0];
        let all = mio::Ready::readable();
        let edge = mio::PollOpt::level();

        assert_eq!(&all, interest);
        assert_eq!(&edge, poll_opt);
    }

    #[test]
    fn rx_close_should_deregister_and_cause_a_transition_to_dead() {
        let stub = TestStepStream::new();
        let state = box HandshakeRx::new(stub, (1, 1));
        let mut ctx = TestPipeContext::new();
        let new_state = state.close(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(1, ctx.get_deregistrations());

        assert_eq!("Dead", new_state.name());
    }

    #[test]
    fn readable_the_handshake_should_be_received() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let pids = (6, 6);
        let state = box HandshakeRx::new(stub, pids);
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::readable();
        let new_state = state.ready(&mut ctx, events);

        assert_eq!(1, sensor.borrow().get_received_handshakes());
        assert_eq!("Active", new_state.name());
    }
}
