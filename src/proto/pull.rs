// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::endpoint::Pipe;
use core::context::Context;
use super::priolist::Priolist;
use super::pipes::PipeCollection;
use super::{Timeout, PUSH, PULL};
use super::policy::fair_queue;
use io_error::*;

pub struct Pull {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Receiving(EndpointId, Timeout),
    RecvOnHold(Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: PipeCollection,
    fq: Priolist
}

/*****************************************************************************/
/*                                                                           */
/* Pull                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Pull {

    fn apply<F>(&mut self, ctx: &mut dyn Context, transition: F) where F : FnOnce(State, &mut dyn Context, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let was_recv_ready = self.is_recv_ready();
            let new_state = transition(old_state, ctx, &mut self.inner);
            let is_recv_ready = self.is_recv_ready();
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            ctx.check_recv_ready_change(was_recv_ready, is_recv_ready);

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }

}

impl From<Sender<Reply>> for Pull {
    fn from(tx: Sender<Reply>) -> Pull {
        Pull {
            inner: Inner {
                reply_tx: tx,
                pipes: PipeCollection::new(),
                fq: Priolist::new()
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

impl Protocol for Pull {
    fn id(&self)      -> u16 { PULL }
    fn peer_id(&self) -> u16 { PUSH }

    fn add_pipe(&mut self, _: &mut dyn Context, eid: EndpointId, pipe: Pipe) {
        self.inner.add_pipe(eid, pipe)
    }
    fn remove_pipe(&mut self, ctx: &mut dyn Context, eid: EndpointId) -> Option<Pipe> {
        let was_recv_ready = self.is_recv_ready();
        let pipe = self.inner.remove_pipe(eid);
        let is_recv_ready = self.is_recv_ready();

        ctx.check_recv_ready_change(was_recv_ready, is_recv_ready);

        if pipe.is_some() {
            self.apply(ctx, |s, ctx, inner| s.on_pipe_removed(ctx, inner, eid));
        }

        pipe
    }
    fn send(&mut self, ctx: &mut dyn Context, msg: Message, timeout: Timeout) {
        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(msg), timeout))
    }
    fn on_send_ack(&mut self, ctx: &mut dyn Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ack(ctx, inner, eid))
    }
    fn on_send_timeout(&mut self, ctx: &mut dyn Context) {
        self.apply(ctx, |s, ctx, inner| s.on_send_timeout(ctx, inner))
    }
    fn on_send_ready(&mut self, ctx: &mut dyn Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ready(ctx, inner, eid))
    }
    fn on_send_not_ready(&mut self, ctx: &mut dyn Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_not_ready(ctx, inner, eid))
    }
    fn recv(&mut self, ctx: &mut dyn Context, timeout: Timeout) {
        self.apply(ctx, |s, ctx, inner| s.recv(ctx, inner, timeout))
    }
    fn on_recv_ack(&mut self, ctx: &mut dyn Context, eid: EndpointId, msg: Message) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg))
    }
    fn on_recv_timeout(&mut self, ctx: &mut dyn Context) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_timeout(ctx, inner))
    }
    fn on_recv_ready(&mut self, ctx: &mut dyn Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ready(ctx, inner, eid))
    }
    fn on_recv_not_ready(&mut self, ctx: &mut dyn Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_not_ready(ctx, inner, eid))
    }
    fn is_send_ready(&self) -> bool {
        false
    }
    fn is_recv_ready(&self) -> bool {
        self.inner.is_recv_ready()
    }
    fn close(&mut self, ctx: &mut dyn Context) {
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
            State::Receiving(..)  => "Receiving",
            State::RecvOnHold(..) => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut dyn Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Receiving(id, timeout) => {
                if id == eid {
                    State::Idle.recv(ctx, inner, timeout)
                } else {
                    State::Receiving(id, timeout)
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

    fn send(self, ctx: &mut dyn Context, inner: &mut Inner, _: Rc<Message>, timeout: Timeout) -> State {
        inner.send(ctx, timeout);
        self
    }
    fn on_send_ack(self, _: &mut dyn Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
    fn on_send_timeout(self, _: &mut dyn Context, _: &mut Inner) -> State {
        self
    }
    fn on_send_ready(self, _: &mut dyn Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
    fn on_send_not_ready(self, _: &mut dyn Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut dyn Context, inner: &mut Inner, timeout: Timeout) -> State {
        inner.recv(ctx).map_or_else(
            |   | State::RecvOnHold(timeout),
            |eid| State::Receiving(eid, timeout))
    }
    fn on_recv_ack(self, ctx: &mut dyn Context, inner: &mut Inner, eid: EndpointId, msg: Message) -> State {
        match self {
            State::Receiving(id, timeout) => {
                if id == eid {
                    inner.on_recv_ack(ctx, timeout, msg);
                    State::Idle
                } else {
                    State::Receiving(id, timeout)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, _: &mut dyn Context, inner: &mut Inner) -> State {
        inner.on_recv_timeout();

        State::Idle
    }
    fn on_recv_ready(self, ctx: &mut dyn Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_ready(eid);

        match self {
            State::RecvOnHold(timeout) => State::Idle.recv(ctx, inner, timeout),
            any => any
        }
    }
    fn on_recv_not_ready(self, _: &mut dyn Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_not_ready(eid);
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
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.fq.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut dyn Context, timeout: Timeout) {
        let error = other_io_error("Send is not supported by pull protocol");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }

    fn recv(&mut self, ctx: &mut dyn Context) -> Option<EndpointId> {
        fair_queue::recv(&mut self.fq, &mut self.pipes, ctx)
    }
    fn on_recv_ready(&mut self, eid: EndpointId) {
        self.fq.activate(&eid)
    }
    fn on_recv_not_ready(&mut self, eid: EndpointId) {
        self.fq.deactivate(&eid)
    }
    fn on_recv_ack(&self, ctx: &mut dyn Context, timeout: Timeout, msg: Message) {
        let _ = self.reply_tx.send(Reply::Recv(msg));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_timeout(&self) {
        let error = timedout_io_error("Recv timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn is_recv_ready(&self) -> bool {
        self.fq.peek()
    }
    fn close(&mut self, ctx: &mut dyn Context) {
        self.pipes.close_all(ctx)
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
    fn when_recv_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut pull = Pull::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        pull.add_pipe(&mut ctx, eid, pipe);
        pull.on_recv_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        pull.recv(&mut ctx, Some(timeout));
        pull.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_recv_from(eid);
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn when_recv_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pull = Pull::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        pull.add_pipe(&mut ctx, eid, pipe);
        pull.on_recv_ready(&mut ctx, eid);
        pull.recv(&mut ctx, None);
        pull.on_recv_ack(&mut ctx, eid, Message::new());
        pull.on_recv_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanRecv(true), raised_evts[2]);
    }

    #[test]
    fn when_recv_ack_event_is_raised_if_there_is_another_pipe_ready() {
        let (tx, _) = mpsc::channel();
        let mut pull = Pull::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid1 = EndpointId::from(1);
        let pipe1 = new_test_pipe(eid1);
        let eid2 = EndpointId::from(2);
        let pipe2 = new_test_pipe(eid2);

        pull.add_pipe(&mut ctx, eid1, pipe1);
        pull.add_pipe(&mut ctx, eid2, pipe2);
        pull.on_recv_ready(&mut ctx, eid1);
        pull.recv(&mut ctx, None);
        pull.on_recv_ready(&mut ctx, eid2);
        pull.on_recv_ack(&mut ctx, eid1, Message::new());

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanRecv(true), raised_evts[2]);
    }

    #[test]
    fn when_recv_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pull = Pull::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(3);
        let pipe = new_test_pipe(eid);

        pull.add_pipe(&mut ctx, eid, pipe);
        pull.on_recv_ready(&mut ctx, eid);
        pull.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
    }
}