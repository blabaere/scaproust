// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use transport::stream::dead::Dead; 
use transport::stream::active::Active; 
use transport::stream::{ 
    StepStream, 
    PipeState,
    transition_if_ok };
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

    fn send_handshake(&mut self) -> io::Result<()> {
        let pids = self.proto_ids;

        self.stream.send_handshake(pids)
    }
}

impl<T : StepStream> Into<HandshakeRx<T>> for HandshakeTx<T> {
    fn into(self) -> HandshakeRx<T> {
        HandshakeRx::new(self.stream, self.proto_ids)
    }
}

impl<T : StepStream> PipeState<T> for HandshakeTx<T> {
    fn name(&self) -> &'static str {"HandshakeTx"}
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
    fn ready(mut self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>> {
        if events.is_writable() {
            let res = self.send_handshake();

            transition_if_ok::<HandshakeTx<T>, HandshakeRx<T>, T>(self, ctx, res)
        } else {
            self
        }
    }
}

pub struct HandshakeRx<T : StepStream + 'static> {
    stream: T,
    proto_ids: (u16, u16)
}

impl<T : StepStream + 'static> HandshakeRx<T> {
    pub fn new(stream: T, pids: (u16, u16)) -> HandshakeRx<T> {
        HandshakeRx {
            stream: stream,
            proto_ids: pids
        }
    }

    fn recv_handshake(&mut self) -> io::Result<()> {
        let pids = self.proto_ids;

        self.stream.recv_handshake(pids)
    }
}

impl<T : StepStream> Into<Active<T>> for HandshakeRx<T> {
    fn into(self) -> Active<T> {
        Active::new(self.stream)
    }
}

impl<T : StepStream> PipeState<T> for HandshakeRx<T> {
    fn name(&self) -> &'static str {"HandshakeRx"}
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
    fn ready(mut self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>> {
        if events.is_readable() {
            let res = self.recv_handshake();
            
            transition_if_ok::<HandshakeRx<T>, Active<T>, T>(self, ctx, res)
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

    use transport::*;
    use transport::tests::*;
    use transport::stream::*;
    use transport::stream::tests::*;
    use transport::stream::handshake::*;

    #[test]
    fn on_writable_the_handshake_should_be_sent() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let pids = (4, 2);
        let state = box HandshakeTx::new(stream, pids);
        let mut ctx = TestPipeContext::new();
        let events = mio::EventSet::writable();
        let new_state = state.ready(&mut ctx, events);

        assert_eq!(1, sensor.borrow().get_sent_handshakes().len());
        assert_eq!(pids, sensor.borrow().get_sent_handshakes()[0]);

        assert_eq!("HandshakeRx", new_state.name());
    }

    #[test]
    fn readable_the_handshake_should_be_received() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let pids = (6, 6);
        let state = box HandshakeRx::new(stream, pids);
        let mut ctx = TestPipeContext::new();
        let events = mio::EventSet::readable();
        let new_state = state.ready(&mut ctx, events);

        assert_eq!(1, sensor.borrow().get_received_handshakes());
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let ref evt = ctx.get_raised_events()[0];
        let is_opened = match evt {
            &PipeEvt::Opened => true,
            _ => false,
        };

        assert!(is_opened);
    }
}