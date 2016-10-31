// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::Sender;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::endpoint::Pipe;
use core::context::{Context, Event};
use super::priolist::Priolist;
use super::{Timeout, REQ, REP};
use io_error::*;

pub struct Rep {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Receiving(EndpointId, Timeout),
    RecvOnHold(Timeout),
    Active(EndpointId),
    Sending(EndpointId, Rc<Message>, Timeout),
    SendOnHold(EndpointId, Rc<Message>, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    fq: Priolist,
    sd: HashSet<EndpointId>,
    ttl: u8,
    backtrace: Vec<u8>,
    is_device_item: bool
}

/*****************************************************************************/
/*                                                                           */
/* Rep                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Rep {

    fn apply<F>(&mut self, ctx: &mut Context, transition: F) where F : FnOnce(State, &mut Context, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let was_send_ready = old_state.is_send_ready(&self.inner);
            let was_recv_ready = old_state.is_recv_ready(&self.inner);
            let new_state = transition(old_state, ctx, &mut self.inner);
            let is_send_ready = new_state.is_send_ready(&self.inner);
            let is_recv_ready = new_state.is_recv_ready(&self.inner);
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            if was_send_ready != is_send_ready {
                ctx.raise(Event::CanSend(is_send_ready));
            }
            if was_recv_ready != is_recv_ready {
                ctx.raise(Event::CanRecv(is_recv_ready));
            }

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }

    fn is_send_ready(&self) -> bool {
        if let Some(ref state) = self.state {
            state.is_send_ready(&self.inner)
        } else {
            false
        }
    }

    fn is_recv_ready(&self) -> bool {
        if let Some(ref state) = self.state {
            state.is_recv_ready(&self.inner)
        } else {
            false
        }
    }

}

impl From<Sender<Reply>> for Rep {
    fn from(tx: Sender<Reply>) -> Rep {
        Rep {
            inner: Inner::new(tx),
            state: Some(State::Idle)
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

impl Protocol for Rep {
    fn id(&self)      -> u16 { REP }
    fn peer_id(&self) -> u16 { REQ }

    fn add_pipe(&mut self, _: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.inner.add_pipe(eid, pipe)
    }
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe> {
        let was_send_ready = self.is_send_ready();
        let was_recv_ready = self.is_recv_ready();
        let pipe = self.inner.remove_pipe(eid);
        let is_send_ready = self.is_send_ready();
        let is_recv_ready = self.is_recv_ready();

        if pipe.is_some() {
            self.apply(ctx, |s, ctx, inner| s.on_pipe_removed(ctx, inner, eid));
        }

        if was_send_ready != is_send_ready {
            ctx.raise(Event::CanSend(is_send_ready));
        }
        if was_recv_ready != is_recv_ready {
            ctx.raise(Event::CanRecv(is_recv_ready));
        }

        pipe
    }
    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Timeout) {
        let raw_msg = self.inner.msg_to_raw_msg(msg);

        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(raw_msg), timeout))
    }
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.inner.clear_backtrace();
        
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
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, raw_msg: Message) {
        if let Some(msg) = self.inner.raw_msg_to_msg(raw_msg) {
            self.inner.set_backtrace(msg.get_header());

            self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg))
        } else {
            self.inner.on_recv_ack_malformed(ctx)
        }
    }
    fn on_recv_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_timeout(ctx, inner))
    }
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ready(ctx, inner, eid))
    }
    fn on_device_plugged(&mut self, _: &mut Context) {
        self.inner.is_device_item = true;
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
            State::Active(..)     => "Active",
            State::Receiving(..)  => "Receiving",
            State::RecvOnHold(..) => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
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

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout) -> State {
        if let State::Active(eid) = self {
            State::Idle.send_reply_to(ctx, inner, msg, timeout, eid)
        } else {
            inner.send_when_inactive(ctx, timeout);

            State::Idle
        }
    }
    fn send_reply_to(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout, eid: EndpointId) -> State {
        if inner.send_to(ctx, msg.clone(), eid) {
            State::Sending(eid, msg, timeout)
        } else {
            State::SendOnHold(eid, msg, timeout)
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
    fn on_send_ready(self, _: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_send_ready(eid);
        self
    }
    fn is_send_ready(&self, inner: &Inner) -> bool {
        if let State::Active(ref eid) = *self {
            inner.is_send_ready(eid)
        } else {
            false
        }
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout) -> State {
        inner.recv(ctx).map_or_else(
            |   | State::RecvOnHold(timeout),
            |eid| State::Receiving(eid, timeout))
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message) -> State {
        match self {
            State::Receiving(id, timeout) => {
                if id == eid {
                    inner.on_recv_ack(ctx, timeout, msg);
                    State::Active(eid)
                } else {
                    State::Receiving(id, timeout)
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
            any => any
        }
    }
    fn is_recv_ready(&self, inner: &Inner) -> bool {
        inner.is_recv_ready()
    }
}

/*****************************************************************************/
/*                                                                           */
/* Inner                                                                     */
/*                                                                           */
/*****************************************************************************/

impl Inner {
    fn new(tx: Sender<Reply>) -> Inner {
        Inner {
            reply_tx: tx,
            pipes: HashMap::new(),
            fq: Priolist::new(),
            sd: HashSet::new(),
            ttl: 8,
            backtrace: Vec::new(),
            is_device_item: false
        }
    }
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.fq.remove(&eid);
        self.sd.remove(&eid);
        self.pipes.remove(&eid)
    }

    fn send_to(&mut self, ctx: &mut Context, msg: Rc<Message>, eid: EndpointId) -> bool {
        self.sd.remove(&eid);
        self.pipes.get_mut(&eid).map(|pipe| pipe.send(ctx, msg)).is_some()
    }
    fn on_send_ack(&self, ctx: &mut Context, timeout: Timeout) {
        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn send_when_inactive(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Can't send: no active request");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        self.sd.insert(eid);
    }
    fn is_send_ready(&self, eid: &EndpointId) -> bool {
        self.sd.contains(eid)
    }

    fn recv(&mut self, ctx: &mut Context) -> Option<EndpointId> {
        self.fq.pop().map_or(None, |eid| self.recv_from(ctx, eid))
    }
    fn recv_from(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.recv(ctx); 
            Some(eid)
        })
    }
    fn on_recv_ready(&mut self, eid: EndpointId) {
        self.fq.activate(&eid)
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
    fn on_recv_ack_malformed(&self, _: &mut Context) {
        let error = invalid_data_io_error("Received request without id");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn is_recv_ready(&self) -> bool {
        self.fq.peek()
    }
 
    fn raw_msg_to_msg(&self, raw_msg: Message) -> Option<Message> {
        if self.is_device_item {
            Some(raw_msg)
        } else {
            decode(raw_msg, self.ttl)
        }
    }
    fn msg_to_raw_msg(&self, msg: Message) -> Message {
        if self.is_device_item {
            msg
        } else {
            encode(msg, self.get_backtrace())
        }
    }
    fn set_backtrace(&mut self, bt: &[u8]) {
        self.backtrace.clear();
        self.backtrace.extend_from_slice(bt);
    }
    fn get_backtrace(&self) -> &[u8] {
        &self.backtrace
    }
    fn clear_backtrace(&mut self) {
        self.backtrace.clear();
    }
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }
}

fn decode(raw_msg: Message, ttl: u8) -> Option<Message> {
    let (mut header, mut body) = raw_msg.split();
    let mut hops = 0;

    loop {
        if hops >= ttl {
            return None;
        }
        hops += 1;

        if body.len() < 4 {
            return None;
        }

        let tail = body.split_off(4);
        header.extend_from_slice(&body);

        let position = header.len() - 4;
        if header[position] & 0x80 != 0 {
            return Some(Message::from_header_and_body(header, tail));
        }
        body = tail;
    }
}

fn encode(msg: Message, backtrace: &[u8]) -> Message {
    let (mut header, body) = msg.split();

    header.extend_from_slice(backtrace);

    Message::from_header_and_body(header, body)
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

    use core::{EndpointId, Message};
    use core::socket::{Protocol, Reply};
    use core::context::{Event, Scheduled};
    use core::tests::*;

    use super::*;

    #[test]
    fn when_recv_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        rep.recv(&mut ctx, Some(timeout));
        rep.on_recv_ack(&mut ctx, eid, msg);

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
    fn send_before_recv_notifies_an_error() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        rep.send(&mut ctx, msg, Some(timeout));
        rep.on_send_ack(&mut ctx, eid);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_err = match reply {
            Reply::Err(..) => true,
            _ => false
        };
        assert!(is_reply_err);

        let sensor = ctx_sensor.borrow();
        sensor.assert_no_send_call();
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_send_ready(&mut ctx, eid);

        let timeout = Scheduled::from(1);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, Message::new());
        let _ = rx.recv(); // flush recv reply

        rep.send(&mut ctx, Message::new(), Some(timeout));
        rep.on_send_ack(&mut ctx, eid);

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
    fn when_recv_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, Message::new());
        rep.on_recv_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanRecv(true), raised_evts[2]);
    }

    #[test]
    fn when_send_starts_event_is_raised() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);

        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, Message::new());
        let _ = rx.recv(); // flush recv reply

        rep.on_send_ready(&mut ctx, eid);
        rep.send(&mut ctx, Message::new(), None);
        rep.on_send_ack(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(4, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
        assert_eq!(Event::CanSend(false), raised_evts[3]);
    }

    #[test]
    fn when_recv_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(2);
        let pipe = new_test_pipe(eid);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
    }
}