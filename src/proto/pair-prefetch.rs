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
use super::{Timeout, PAIR};
use io_error::*;

pub struct Pair {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Sending(EndpointId, Rc<Message>, Timeout),
    SendOnHold(Rc<Message>, Timeout),
    Receiving(EndpointId, Timeout, bool),
    RecvOnHold(Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipe: Option<(EndpointId, Pipe)>,
    prefetched_msg: Option<Message>,
    send_ready: bool,
    recv_ready: bool
}

/*****************************************************************************/
/*                                                                           */
/* Pair                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Pair {

    fn apply<F>(&mut self, ctx: &mut Context, transition: F) where F : FnOnce(State, &mut Context, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let was_send_ready = self.inner.send_ready;
            let was_recv_ready = self.inner.recv_ready;
            let new_state = transition(old_state, ctx, &mut self.inner);
            let is_send_ready = self.inner.send_ready;
            let is_recv_ready = self.inner.recv_ready;
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            ctx.check_send_ready_change(was_send_ready, is_send_ready);
            ctx.check_recv_ready_change(was_recv_ready, is_recv_ready);

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }

}

#[doc(hidden)]
impl From<Sender<Reply>> for Pair {
    fn from(tx: Sender<Reply>) -> Pair {
        Pair {
            inner: Inner {
                reply_tx: tx,
                pipe: None,
                prefetched_msg: None,
                send_ready: false,
                recv_ready: false
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

impl Protocol for Pair {
    fn id(&self)      -> u16 { PAIR }
    fn peer_id(&self) -> u16 { PAIR }

    fn add_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.inner.add_pipe(ctx, eid, pipe)
    }
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe> {
        let pipe = self.inner.remove_pipe(ctx, eid);

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
        self.inner.send_ready
    }
    fn is_recv_ready(&self) -> bool {
        self.inner.recv_ready
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
            State::SendOnHold(..) => "SendOnHold",
            State::Receiving(..)  => "Receiving",
            State::RecvOnHold(..) => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_pipe_removed(ctx);

        match self {
            State::Sending(id, msg, timeout) => {
                if id == eid {
                    State::SendOnHold(msg, timeout)
                } else {
                    State::Sending(id, msg, timeout)
                }
            },
            State::Receiving(id, timeout, prefetching) => {
                if id == eid {
                    if prefetching {
                        State::Idle
                    } else {
                        State::RecvOnHold(timeout)
                    }
                } else {
                    State::Receiving(id, timeout, prefetching)
                }
            }
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
        match self {
            State::Idle => {
                if let Some(msg) = inner.prefetched_msg.take() {
                    inner.on_recv_ack(ctx, timeout, msg);

                    State::Idle
                } else {
                    inner.recv(ctx).map_or_else(
                        |   | State::RecvOnHold(timeout),
                        |eid| State::Receiving(eid, timeout, false))
                }
            },
            State::Receiving(id, timeout, true) => {
                State::Receiving(id, timeout, false)
            },
            any => any
        }
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message) -> State {
        match self {
            State::Receiving(id, timeout, prefetching) => {
                if id == eid {
                    if prefetching {
                        inner.prefetched_msg = Some(msg);
                    } else {
                        inner.on_recv_ack(ctx, timeout, msg);
                    }
                    
                    State::Idle
                } else {
                    State::Receiving(id, timeout, prefetching)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, _: &mut Context, inner: &mut Inner) -> State {
        inner.on_recv_timeout();

        State::Idle
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_ready(eid);

        match self {
            State::RecvOnHold(timeout) => State::Idle.recv(ctx, inner, timeout),
            State::Idle => {
                if inner.prefetched_msg.is_some() {
                    State::Idle
                } else {
                    inner.recv(ctx).map_or_else(
                        |   | State::Idle,
                        |eid| State::Receiving(eid, None, true))
                }
            },
            any => any
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Inner                                                                     */
/*                                                                           */
/*****************************************************************************/

impl Inner {
    fn add_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe) {
        if self.pipe.is_none() {
            self.pipe = Some((eid, pipe));
        } else {
            pipe.close(ctx);
        }
    }
    fn remove_pipe(&mut self, _: &mut Context, eid: EndpointId) -> Option<Pipe> {
        if let Some((id, pipe)) = self.pipe.take() {
            if id == eid {
                return Some(pipe);
            } else {
                self.pipe = Some((id, pipe));
            }
        }

        None
    }
    fn on_pipe_removed(&mut self, _: &mut Context) {
        self.send_ready = false;
        self.recv_ready = false;
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) -> Option<EndpointId> {
        if self.send_ready == false {
            return None
        }

        self.send_ready = false;
        self.pipe.as_ref().map_or(None, |&(ref eid, ref pipe)| {
            pipe.send(ctx, msg); 
            Some(*eid)
        })
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        if self.pipe.as_ref().map(|&(ref id, _)| *id) == Some(eid) {
            self.send_ready = true;
        }
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

    fn recv(&mut self, ctx: &mut Context) -> Option<EndpointId> {
        if self.recv_ready == false {
            return None
        }
        
        self.recv_ready = false;
        self.pipe.as_ref().map_or(None, |&(ref eid, ref pipe)| {
            pipe.recv(ctx); 
            Some(*eid)
        })
    }
    fn on_recv_ready(&mut self, eid: EndpointId) {
        if self.pipe.as_ref().map(|&(ref id, _)| *id) == Some(eid) {
            self.recv_ready = true;
        }
    }
    fn on_recv_ack(&self, ctx: &mut Context, timeout: Timeout, msg: Message) {
        let _ = self.reply_tx.send(Reply::Recv(msg));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_timeout(&self) {
        let error = timedout_io_error("Recv timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn close(&mut self, ctx: &mut Context) {
        self.pipe.take().map(|(_, pipe)| pipe.close(ctx));
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
    fn adding_more_than_one_pipe_should_close_the_subsequent_ones() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid1 = EndpointId::from(1);
        let pipe1 = new_test_pipe(eid1);
        let eid2 = EndpointId::from(2);
        let pipe2 = new_test_pipe(eid2);

        pair.add_pipe(&mut ctx, eid1, pipe1);
        pair.add_pipe(&mut ctx, eid2, pipe2);

        let sensor = ctx_sensor.borrow();
        let close_calls = sensor.get_close_calls();
        assert_eq!(1, close_calls.len());

        let &(ref eid, ref remote) = &close_calls[0];
        assert_eq!(eid2, *eid);
        assert!(*remote);
    }

    #[test]
    fn remove_returns_the_added_pipe() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(3);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);

        let removed = pair.remove_pipe(&mut ctx, eid);
        assert!(removed.is_some());

        if let Some(pipe) = removed {
            pipe.close(&mut ctx);
        }

        let sensor = ctx_sensor.borrow();
        let close_calls = sensor.get_close_calls();
        assert_eq!(1, close_calls.len());

        let &(ref id, ref remote) = &close_calls[0];
        assert_eq!(eid, *id);
        assert!(*remote);
    }

    #[test]
    fn can_put_send_on_hold_and_resume_when_a_pipe_is_added_and_becomes_ready() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let msg = Message::new();

        pair.send(&mut ctx, msg, None);
        ctx_sensor.borrow().assert_no_send_call();

        let eid = EndpointId::from(4);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        ctx_sensor.borrow().assert_no_send_call();

        pair.on_send_ready(&mut ctx, eid);
        ctx_sensor.borrow().assert_one_send_to(eid);
    }

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(6);
        pair.send(&mut ctx, msg, Some(timeout));
        pair.on_send_ack(&mut ctx, eid);

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
    fn when_send_timeout_is_reached_err_is_notified() {
        let (tx, rx) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(7);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(8);
        pair.send(&mut ctx, msg, Some(timeout));
        pair.on_send_timeout(&mut ctx);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_err = match reply {
            Reply::Err(_) => true,
            _ => false
        };
        assert!(is_reply_err);
    }

    //#[test]
    fn when_recv_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(9);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_recv_ready(&mut ctx, eid);

        let timeout = Scheduled::from(0);
        pair.recv(&mut ctx, Some(timeout));
        pair.on_recv_ack(&mut ctx, eid, Message::new());

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
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
    fn can_put_recv_on_hold_and_resume_when_a_pipe_is_added_and_becomes_ready() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());

        pair.recv(&mut ctx, None);
        ctx_sensor.borrow().assert_no_recv_call();

        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        ctx_sensor.borrow().assert_no_recv_call();

        pair.on_recv_ready(&mut ctx, eid);
        ctx_sensor.borrow().assert_one_recv_from(eid);
    }

    #[test]
    fn when_recv_timeout_is_reached_err_is_notified() {
        let (tx, rx) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(2);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);

        let timeout = Scheduled::from(3);
        pair.recv(&mut ctx, Some(timeout));
        pair.on_recv_timeout(&mut ctx);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_err = match reply {
            Reply::Err(_) => true,
            _ => false
        };
        assert!(is_reply_err);
    }

    #[test]
    fn when_peer_becomes_writable_can_send_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(4);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_send_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(1, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
    }

    #[test]
    fn when_send_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_send_ready(&mut ctx, eid);
        pair.send(&mut ctx, Message::new(), None);
        pair.on_send_ack(&mut ctx, eid);
        pair.on_send_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
    }

    //#[test]
    fn when_recv_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(6);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_recv_ready(&mut ctx, eid);
        pair.recv(&mut ctx, None);
        pair.on_recv_ack(&mut ctx, eid, Message::new());
        pair.on_recv_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanRecv(true), raised_evts[2]);
    }

    #[test]
    fn when_send_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_send_ready(&mut ctx, eid);
        pair.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
    }

    //#[test]
    fn when_recv_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut pair = Pair::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);

        pair.add_pipe(&mut ctx, eid, pipe);
        pair.on_recv_ready(&mut ctx, eid);
        pair.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
    }
}
