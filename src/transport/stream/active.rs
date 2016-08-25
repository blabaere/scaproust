// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use transport::stream::dead::Dead; 
use transport::stream::{ 
    StepStream, 
    PipeState,
    no_transition_if_ok };
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
    fn on_send_progress(&mut self, ctx: &mut Context<PipeEvt>, progress: io::Result<bool>) -> io::Result<()> {
        progress.map(|sent| if sent { self.on_msg_sent(ctx) } )
    }
    fn on_msg_sent(&mut self, ctx: &mut Context<PipeEvt>) {
        ctx.raise(PipeEvt::Sent);
        ctx.reregister(self.stream.deref(), mio::EventSet::all(), mio::PollOpt::edge());
    }

    fn writable_changed(&mut self, ctx: &mut Context<PipeEvt>, writable: bool) -> io::Result<()> {
        if writable {
            if self.stream.has_pending_send() {
                let progress = self.stream.resume_send();

                self.on_send_progress(ctx, progress)
            } else {
                ctx.raise(PipeEvt::CanSend);

                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn on_recv_progress(&mut self, ctx: &mut Context<PipeEvt>, progress: io::Result<Option<Message>>) -> io::Result<()> {
        progress.map(|recv| if let Some(msg) = recv { self.on_msg_received(ctx, msg) } )
    }

    fn on_msg_received(&mut self, ctx: &mut Context<PipeEvt>, msg: Message) {
        ctx.raise(PipeEvt::Received(msg));
        ctx.reregister(self.stream.deref(), mio::EventSet::all(), mio::PollOpt::edge());
    }

    fn readable_changed(&mut self, ctx: &mut Context<PipeEvt>, readable: bool) -> io::Result<()> {
        if readable {
            if self.stream.has_pending_recv() {
                let progress = self.stream.resume_recv();

                self.on_recv_progress(ctx, progress)
            } else {
                ctx.raise(PipeEvt::CanRecv);
                
                Ok(())
            }
        } else {
            Ok(())
        }
    }
}

impl<T : StepStream> PipeState<T> for Active<T> {
    fn name(&self) -> &'static str {"Active"}

    fn enter(&self, ctx: &mut Context<PipeEvt>) {
        ctx.reregister(self.stream.deref(), mio::EventSet::all(), mio::PollOpt::edge());
        ctx.raise(PipeEvt::Opened);
    }
    fn close(self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        ctx.deregister(self.stream.deref());

        box Dead
    }
    fn send(mut self: Box<Self>, ctx: &mut Context<PipeEvt>, msg: Rc<Message>) -> Box<PipeState<T>> {
        let progress = self.stream.start_send(msg);
        let res = self.on_send_progress(ctx, progress);

        no_transition_if_ok(self, ctx, res)
    }
    fn recv(mut self: Box<Self>, ctx: &mut Context<PipeEvt>) -> Box<PipeState<T>> {
        let progress = self.stream.start_recv();
        let res = self.on_recv_progress(ctx, progress);

        no_transition_if_ok(self, ctx, res)
    }
    fn ready(mut self: Box<Self>, ctx: &mut Context<PipeEvt>, events: mio::EventSet) -> Box<PipeState<T>> {
        let res = 
            self.readable_changed(ctx, events.is_readable()).and_then(|_|
            self.writable_changed(ctx, events.is_writable()));

        no_transition_if_ok(self, ctx, res)
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
    use transport::stream::active::*;
    use Message;

    #[test]
    fn on_enter_stream_is_reregistered_and_an_event_is_raised() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let state = box Active::new(stream);
        let mut ctx = TestPipeContext::new();

        state.enter(&mut ctx);

        assert_eq!(0, ctx.get_registrations().len());
        assert_eq!(1, ctx.get_reregistrations().len());
        assert_eq!(0, ctx.get_deregistrations());

        let (ref interest, ref poll_opt) = ctx.get_reregistrations()[0];
        let all = mio::EventSet::all();
        let edge = mio::PollOpt::edge();

        assert_eq!(&all, interest);
        assert_eq!(&edge, poll_opt);

        assert_eq!(1, ctx.get_raised_events().len());
        let ref evt = ctx.get_raised_events()[0];
        let is_opened = match evt {
            &PipeEvt::Opened => true,
            _ => false,
        };

        assert!(is_opened);
    }

    #[test]
    fn close_should_deregister_and_cause_a_transition_to_dead() {
        let stream = TestStepStream::new();
        let state = box Active::new(stream);
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
        let stream = TestStepStream::with_sensor(sensor.clone());
        let state = box Active::new(stream);
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Rc::new(Message::from_body(payload));
        let new_state = state.send(&mut ctx, msg);

        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let ref evt = ctx.get_raised_events()[0];
        let is_sent = match evt {
            &PipeEvt::Sent => true,
            _ => false,
        };

        assert!(is_sent);
    }

    #[test]
    fn send_with_postponed_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let state = box Active::new(stream);
        let mut ctx = TestPipeContext::new();

        sensor.borrow_mut().set_start_send_result(Some(false));
        let payload = vec!(66, 65, 67);
        let msg = Rc::new(Message::from_body(payload));
        let new_state = state.send(&mut ctx, msg);

        assert_eq!("Active", new_state.name());
        assert_eq!(0, ctx.get_raised_events().len());

        sensor.borrow_mut().set_resume_send_result(Some(true));
        let events = mio::EventSet::writable();
        let new_state = new_state.ready(&mut ctx, events);

        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let ref evt = ctx.get_raised_events()[0];
        let is_sent = match evt {
            &PipeEvt::Sent => true,
            _ => false,
        };

        assert!(is_sent);
    }

    #[test]
    fn recv_with_immediate_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let state = box Active::new(stream);
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Message::from_body(payload);

        sensor.borrow_mut().set_start_recv_result(Some(msg));
        let new_state = state.recv(&mut ctx);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let ref evt = ctx.get_raised_events()[0];
        let is_recv = match evt {
            &PipeEvt::Received(_) => true,
            _ => false,
        };

        assert!(is_recv);
    }

    #[test]
    fn recv_with_postponed_success() {
        let sensor_srv = TestStepStreamSensor::new();
        let sensor = Rc::new(RefCell::new(sensor_srv));
        let stream = TestStepStream::with_sensor(sensor.clone());
        let state = box Active::new(stream);
        let mut ctx = TestPipeContext::new();
        let payload = vec!(66, 65, 67);
        let msg = Message::from_body(payload);

        sensor.borrow_mut().set_start_recv_result(None);
        let new_state = state.recv(&mut ctx);
        assert_eq!("Active", new_state.name());
        assert_eq!(0, ctx.get_raised_events().len());

        sensor.borrow_mut().set_resume_recv_result(Some(msg));
        let events = mio::EventSet::readable();
        let new_state = new_state.ready(&mut ctx, events);
        assert_eq!("Active", new_state.name());
        assert_eq!(1, ctx.get_raised_events().len());

        let ref evt = ctx.get_raised_events()[0];
        let is_recv = match evt {
            &PipeEvt::Received(_) => true,
            _ => false,
        };

        assert!(is_recv);
    }
}