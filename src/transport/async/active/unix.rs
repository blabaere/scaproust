// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io::Result;

use mio::{Ready, PollOpt};

use core::Message;
use transport::async::stub::*;
use transport::async::state::*;
use transport::async::dead::Dead; 
use transport::pipe::{Event, Context};
use io_error::*;

// pipe readiness value is different from the underlying I/O readiness
// because while an operation is in progress, must be finished
// before being able to start another one. 

pub struct Active<S> {
    stub: S,
    can_send_msg: bool,
    can_recv_msg: bool
}

impl<S : AsyncPipeStub> Active<S> {
    pub fn new(s: S) -> Active<S> {
        Active {
            stub: s,
            can_send_msg: false,
            can_recv_msg: false
        }
    }
    fn on_send_progress(&mut self, ctx: &mut Context, progress: Result<bool>) -> Result<()> {
        progress.map(|sent| if sent { self.on_msg_sent(ctx) } )
    }
    fn on_msg_sent(&mut self, ctx: &mut Context) {
        ctx.raise(Event::Sent);
    }
    fn writable_changed(&mut self, ctx: &mut Context, events: Ready) -> Result<()> {
        if events.is_writable() == false {
            return Ok(self.change_can_send(ctx, false));
        }

        if self.stub.has_pending_send() {
            let progress = self.stub.resume_send();
            return self.on_send_progress(ctx, progress);
        }

        if events.is_hup() == false {
            return Ok(self.change_can_send(ctx, true));
        }

        Ok(())
    }
    fn change_can_send(&mut self, ctx: &mut Context, can_send: bool) {
        if self.can_send_msg != can_send {
            self.can_send_msg = can_send;
            ctx.raise(Event::CanSend(can_send));
        }
    }

    fn on_recv_progress(&mut self, ctx: &mut Context, progress: Result<Option<Message>>) -> Result<()> {
        progress.map(|recv| if let Some(msg) = recv { self.on_msg_received(ctx, msg) } )
    }
    fn on_msg_received(&mut self, ctx: &mut Context, msg: Message) {
        ctx.raise(Event::Received(msg));
    }
    fn readable_changed(&mut self, ctx: &mut Context, events: Ready) -> Result<()> {
        if events.is_readable() == false {
            return Ok(self.change_can_recv(ctx, false));
        }

        if self.stub.has_pending_recv() {
            let progress = self.stub.resume_recv();
            return self.on_recv_progress(ctx, progress);
        }

        if events.is_hup() == false {
            return Ok(self.change_can_recv(ctx, true));
        }

        Ok(())
    }
    fn change_can_recv(&mut self, ctx: &mut Context, can_recv: bool) {
        if self.can_recv_msg != can_recv {
            self.can_recv_msg = can_recv;
            ctx.raise(Event::CanRecv(can_recv));
        }
    }

    fn hang_up_changed(&mut self, hup: bool) -> Result<()> {
        if hup {
            self.can_send_msg = false;
            self.can_recv_msg = false;
            Err(other_io_error("hup"))
        } else {
            Ok(())
        }
    }
}

impl<S : AsyncPipeStub + 'static> PipeState<S> for Active<S> {
    fn name(&self) -> &'static str {"Active"}

    fn enter(&mut self, ctx: &mut Context) {
        ctx.reregister(self.stub.deref(), Ready::all(), PollOpt::level());
        ctx.raise(Event::Opened);
    }
    fn close(self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        ctx.deregister(self.stub.deref());

        Box::new(Dead)
    }
    fn send(mut self: Box<Self>, ctx: &mut Context, msg: Rc<Message>) -> Box<PipeState<S>> {
        self.can_send_msg = false;

        let progress = self.stub.start_send(msg);
        let res = self.on_send_progress(ctx, progress);

        no_transition_if_ok(self, ctx, res)
    }
    fn recv(mut self: Box<Self>, ctx: &mut Context) -> Box<PipeState<S>> {
        self.can_recv_msg = false;

        let progress = self.stub.start_recv();
        let res = self.on_recv_progress(ctx, progress);

        no_transition_if_ok(self, ctx, res)
    }
    fn ready(mut self: Box<Self>, ctx: &mut Context, events: Ready) -> Box<PipeState<S>> {
        let res = 
            self.readable_changed(ctx, events).and_then(|_|
            self.writable_changed(ctx, events).and_then(|_| 
            self.hang_up_changed(events.is_hup()))
        );

        no_transition_if_ok(self, ctx, res)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::cell::RefCell;

    use mio;

    use core::Message;
    use transport::*;
    use transport::tests::*;
    use transport::async::state::*;
    use transport::async::tests::*;
    use transport::async::active::*;

    #[test]
    fn on_enter_stub_is_reregistered_and_an_event_is_raised() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let mut state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();

        state.enter(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(1, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        let (ref interest, ref poll_opt) = ctx.get_reregistrations()[0];
        let all = mio::Ready::all();
        let level = mio::PollOpt::level();

        assert_eq!(&all, interest);
        assert_eq!(&level, poll_opt);

        assert_eq!(1, ctx.get_raised_events().len());
        let evt = &ctx.get_raised_events()[0];
        let is_opened = match *evt {
            pipe::Event::Opened => true,
            _ => false,
        };

        assert!(is_opened);
    }

    #[test]
    fn close_should_deregister_and_cause_a_transition_to_dead() {
        let stub = TestStepStream::new();
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let new_state = state.close(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(0, ctx.get_reregistrations().len());
        assert_eq!(1, ctx.get_deregistrations());

        assert_eq!("Dead", new_state.name());
    }

    #[test]
    fn send_with_immediate_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Rc::new(Message::from_body(payload));
        let new_state = state.send(&mut ctx, msg);

        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_sent = match *evt {
            pipe::Event::Sent => true,
            _ => false,
        };
        assert!(is_sent);
    }

    #[test]
    fn send_with_postponed_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();

        sensor.borrow_mut().set_start_send_result(Some(false));
        let payload = vec!(66, 65, 67);
        let msg = Rc::new(Message::from_body(payload));
        let new_state = state.send(&mut ctx, msg);

        assert_eq!("Active", new_state.name());
        assert_eq!(0, ctx.get_raised_events().len());

        sensor.borrow_mut().set_resume_send_result(Some(true));
        let events = mio::Ready::writable();
        let new_state = new_state.ready(&mut ctx, events);

        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_sent = match *evt {
            pipe::Event::Sent => true,
            _ => false,
        };
        assert!(is_sent);
    }

    #[test]
    fn when_writable_should_raise_an_event() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::writable();
        let new_state = state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_can_send = match *evt {
            pipe::Event::CanSend(x) => x,
            _ => false,
        };

        assert!(is_can_send);
    }

    #[test]
    fn when_writable_should_raise_an_event_unless_no_change() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::writable();
        let new_state = state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let new_state = new_state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());
    }

    #[test]
    fn when_writable_hup_should_not_raise_an_event_and_transition_to_dead() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::writable() | mio::Ready::hup();
        let new_state = state.ready(&mut ctx, events);
        assert_eq!("Dead", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_error = match *evt {
            pipe::Event::Error(_) => true,
            _ => false,
        };

        assert!(is_error);
    }

    #[test]
    fn recv_with_immediate_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Message::from_body(payload);

        sensor.borrow_mut().set_start_recv_result(Some(msg));
        let new_state = state.recv(&mut ctx);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_recv = match *evt {
            pipe::Event::Received(_) => true,
            _ => false,
        };

        assert!(is_recv);
    }

    #[test]
    fn recv_with_postponed_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Message::from_body(payload);

        sensor.borrow_mut().set_start_recv_result(None);
        let new_state = state.recv(&mut ctx);
        assert_eq!("Active", new_state.name());
        assert_eq!(0, ctx.get_raised_events().len());

        sensor.borrow_mut().set_resume_recv_result(Some(msg));
        let events = mio::Ready::readable();
        let new_state = new_state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_recv = match *evt {
            pipe::Event::Received(_) => true,
            _ => false,
        };

        assert!(is_recv);
    }

    #[test]
    fn when_readable_should_raise_an_event() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::readable();
        let new_state = state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_can_recv = match *evt {
            pipe::Event::CanRecv(x) => x,
            _ => false,
        };

        assert!(is_can_recv);
    }

    #[test]
    fn when_readable_hup_should_not_raise_an_event_and_transition_to_dead() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stub = TestStepStream::with_sensor(sensor.clone());
        let state = Box::new(Active::new(stub));
        let mut ctx = TestPipeContext::new();
        let events = mio::Ready::readable() | mio::Ready::hup();
        let new_state = state.ready(&mut ctx, events);
        assert_eq!("Dead", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let evt = &ctx.get_raised_events()[0];
        let is_error = match *evt {
            pipe::Event::Error(_) => true,
            _ => false,
        };

        assert!(is_error);
    }
}