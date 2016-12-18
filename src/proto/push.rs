// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::endpoint::Pipe;
use core::context::Context;
use super::priolist::Priolist;
use super::{Timeout, PUSH, PULL};
use io_error::*;

pub struct Push {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Sending(EndpointId, Rc<Message>, Timeout),
    SendOnHold(Rc<Message>, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    lb: Priolist
}

/*****************************************************************************/
/*                                                                           */
/* Push                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Push {

    fn apply<F>(&mut self, ctx: &mut Context, transition: F) where F : FnOnce(State, &mut Context, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let was_send_ready = self.is_send_ready();
            let new_state = transition(old_state, ctx, &mut self.inner);
            let is_send_ready = self.is_send_ready();
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            ctx.check_send_ready_change(was_send_ready, is_send_ready);

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }

}

impl From<Sender<Reply>> for Push {
    fn from(tx: Sender<Reply>) -> Push {
        Push {
            inner: Inner {
                reply_tx: tx,
                pipes: HashMap::new(),
                lb: Priolist::new()
            },
            state: Some(State::Idle)
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

impl Protocol for Push {
    fn id(&self)      -> u16 { PUSH }
    fn peer_id(&self) -> u16 { PULL }

    fn add_pipe(&mut self, _: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.inner.add_pipe(eid, pipe)
    }
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe> {
        let was_send_ready = self.is_send_ready();
        let pipe = self.inner.remove_pipe(eid);
        let is_send_ready = self.is_send_ready();

        ctx.check_send_ready_change(was_send_ready, is_send_ready);

        if pipe.is_some() {
            self.apply(ctx, |s, ctx, inner| s.on_pipe_removed(ctx, inner, eid));
        }

        pipe
    }
    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Timeout) {
        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(msg), timeout))
    }
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ack(ctx, inner, eid))
    }
    fn on_send_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_send_timeout(ctx, inner))
    }
    fn on_send_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ready(ctx, inner, eid))
    }
    fn recv(&mut self, ctx: &mut Context, timeout: Timeout) {
        self.apply(ctx, |s, ctx, inner| s.recv(ctx, inner, timeout))
    }
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg))
    }
    fn on_recv_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_timeout(ctx, inner))
    }
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ready(ctx, inner, eid))
    }
    fn is_send_ready(&self) -> bool {
        self.inner.is_send_ready()
    }
    fn is_recv_ready(&self) -> bool {
        false
    }
    fn close(&mut self, ctx: &mut Context) {
        self.inner.close(ctx)
    }
}

/*****************************************************************************/
/*                                                                           */
/* State                                                                     */
/*                                                                           */
/*****************************************************************************/

impl State {

    #[cfg(debug_assertions)]
    fn name(&self) -> &'static str {
        match *self {
            State::Idle           => "Idle",
            State::Sending(..)    => "Sending",
            State::SendOnHold(..) => "SendOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, msg, timeout) => {
                if id == eid {
                    State::Idle.send(ctx, inner, msg, timeout)
                } else {
                    State::Sending(id, msg, timeout)
                }
            },
            any => any
        }
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout) -> State {
        if let Some(eid) = inner.send(ctx, msg.clone()) {
            State::Sending(eid, msg, timeout)
        } else {
            State::SendOnHold(msg, timeout)
        }
    }
    fn on_send_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, msg, timeout) => {
                if id == eid {
                    inner.on_send_ack(ctx, timeout);
                    State::Idle
                } else {
                    State::Sending(id, msg, timeout)
                }
            },
            any => any
        }
    }
    fn on_send_timeout(self, _: &mut Context, inner: &mut Inner) -> State {
        inner.on_send_timeout();

        State::Idle
    }
    fn on_send_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_send_ready(eid);

        match self {
            State::SendOnHold(msg, timeout) => State::Idle.send(ctx, inner, msg, timeout),
            any => any
        }
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout) -> State {
        inner.recv(ctx, timeout);
        self
    }
    fn on_recv_ack(self, _: &mut Context, _: &mut Inner, _: EndpointId, _: Message) -> State {
        self
    }
    fn on_recv_timeout(self, _: &mut Context, _: &mut Inner) -> State {
        self
    }
    fn on_recv_ready(self, _: &mut Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
}

/*****************************************************************************/
/*                                                                           */
/* Inner                                                                     */
/*                                                                           */
/*****************************************************************************/

impl Inner {
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.lb.insert(eid, pipe.get_send_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.lb.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) -> Option<EndpointId> {
        self.lb.pop().map_or(None, |eid| self.send_to(ctx, msg, eid))
    }
    fn send_to(&mut self, ctx: &mut Context, msg: Rc<Message>, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.send(ctx, msg); 
            Some(eid)
        })
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        self.lb.activate(&eid)
    }
    fn on_send_ack(&self, ctx: &mut Context, timeout: Timeout) {
        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn is_send_ready(&self) -> bool {
        self.lb.peek()
    }

    fn recv(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Recv is not supported by push protocol");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* tests                                                                     */
/*                                                                           */
/*****************************************************************************/

#[cfg(test)]
mod tests {

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc;

    use core::{EndpointId, Message, Scheduled};
    use core::socket::{Protocol, Reply};
    use core::context::{Event};
    use core::tests::*;

    use super::*;

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut push = Push::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        push.add_pipe(&mut ctx, eid, pipe);
        push.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        push.send(&mut ctx, msg, Some(timeout));
        push.on_send_ack(&mut ctx, eid);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Send => true,
            _ => false
        };
        assert!(is_reply_ok);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_send_to(eid);
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn when_send_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut push = Push::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        push.add_pipe(&mut ctx, eid, pipe);
        push.on_send_ready(&mut ctx, eid);
        push.send(&mut ctx, Message::new(), None);
        push.on_send_ack(&mut ctx, eid);
        push.on_send_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
    }

    #[test]
    fn when_send_ack_event_is_raised_if_there_is_another_pipe_ready() {
        let (tx, _) = mpsc::channel();
        let mut push = Push::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid1 = EndpointId::from(1);
        let pipe1 = new_test_pipe(eid1);
        let eid2 = EndpointId::from(2);
        let pipe2 = new_test_pipe(eid2);

        push.add_pipe(&mut ctx, eid1, pipe1);
        push.add_pipe(&mut ctx, eid2, pipe2);
        push.on_send_ready(&mut ctx, eid1);
        push.send(&mut ctx, Message::new(), None);
        push.on_send_ready(&mut ctx, eid2);
        push.on_send_ack(&mut ctx, eid1);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
    }

    #[test]
    fn when_send_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut push = Push::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(3);
        let pipe = new_test_pipe(eid);

        push.add_pipe(&mut ctx, eid, pipe);
        push.on_send_ready(&mut ctx, eid);
        push.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
    }

}