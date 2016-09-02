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
            let new_state = transition(old_state, ctx, &mut self.inner);
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

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
        let pipe = self.inner.remove_pipe(eid);

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
            State::Idle             => "Idle",
            State::Sending(_, _, _) => "Sending",
            State::SendOnHold(_, _) => "SendOnHold"
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
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
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
