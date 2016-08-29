// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
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
use core::context::{Context, Scheduled};
use super::priolist::Priolist;
use super::{Timeout, PUSH, PULL};
use io_error::*;

pub struct Push {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Sending(EndpointId, Timeout),
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

    fn apply<F>(&mut self, transition: F) where F : FnOnce(State, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            let old_name = old_state.name();
            let new_state = transition(old_state, &mut self.inner);
            let new_name = new_state.name();

            self.state = Some(new_state);
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
    fn remove_pipe(&mut self, _: &mut Context, eid: EndpointId) -> Option<Pipe> {
        self.inner.remove_pipe(eid)
    }

    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Option<Scheduled>) {
        self.apply(|s, inner| s.send(ctx, inner, Rc::new(msg), timeout))
    }
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(|s, inner| s.on_send_ack(ctx, inner, eid))
    }
    fn on_send_timeout(&mut self, ctx: &mut Context) {
        self.apply(|s, inner| s.on_send_timeout(ctx, inner))
    }
    fn on_send_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(|s, inner| s.on_send_ready(ctx, inner, eid))
    }

    fn recv(&mut self, ctx: &mut Context, timeout: Option<Scheduled>) {
    }
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message) {
    }
    fn on_recv_timeout(&mut self, ctx: &mut Context) {
    }
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
    }
}

/*****************************************************************************/
/*                                                                           */
/* State                                                                     */
/*                                                                           */
/*****************************************************************************/

impl State {

    fn name(&self) -> &'static str {
        match *self {
            State::Idle             => "Idle",
            State::Sending(_, _)    => "Sending",
            State::SendOnHold(_, _) => "SendOnHold"
        }
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Option<Scheduled>) -> State {
        inner.send(ctx, msg.clone()).map_or_else(
            |   | State::SendOnHold(msg, timeout),
            |eid| State::Sending(eid, timeout))
    }
    fn on_send_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, timeout) => {
                if id == eid {
                    inner.on_send_ack(ctx, timeout);
                    State::Idle
                } else {
                    State::Sending(id, timeout)
                }
            },
            any => any
        }
    }
    fn on_send_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        inner.on_send_timeout(ctx);

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

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Option<Scheduled>) -> State {
        self
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message) -> State {
        self
    }
    fn on_recv_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        self
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
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
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) -> Option<EndpointId> {
        self.lb.next().map_or(None, |eid| self.send_to(ctx, msg, eid))
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
    fn on_send_timeout(&self, ctx: &mut Context) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
}
