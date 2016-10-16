// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::config::ConfigOption;
use core::endpoint::Pipe;
use core::context::{Context, Event};
use super::priolist::Priolist;
use super::{Timeout, PUB, SUB};
use io_error::*;

pub struct Sub {
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
    pipes: HashMap<EndpointId, Pipe>,
    fq: Priolist,
    subscriptions: HashSet<Vec<u8>>
}

/*****************************************************************************/
/*                                                                           */
/* Sub                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Sub {

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

impl From<Sender<Reply>> for Sub {
    fn from(tx: Sender<Reply>) -> Sub {
        Sub {
            inner: Inner {
                reply_tx: tx,
                pipes: HashMap::new(),
                fq: Priolist::new(),
                subscriptions: HashSet::new()
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

impl Protocol for Sub {
    fn id(&self)      -> u16 { SUB }
    fn peer_id(&self) -> u16 { PUB }

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
    fn set_option(&mut self, opt: ConfigOption) -> io::Result<()> {
        match opt {
            ConfigOption::Subscribe(x)   => Ok(self.inner.subscribe(x)),
            ConfigOption::Unsubscribe(x) => Ok(self.inner.unsubscribe(x)),
            _ => Err(invalid_input_io_error("option not supported"))
        }
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
            State::Idle            => "Idle",
            State::Receiving(_, _) => "Receiving",
            State::RecvOnHold(_)   => "RecvOnHold"
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

    fn send(self, ctx: &mut Context, inner: &mut Inner, _: Rc<Message>, timeout: Timeout) -> State {
        inner.send(ctx, timeout);
        self
    }
    fn on_send_ack(self, _: &mut Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
    fn on_send_timeout(self, _: &mut Context, _: &mut Inner) -> State {
        self
    }
    fn on_send_ready(self, _: &mut Context, _: &mut Inner, _: EndpointId) -> State {
        self
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
                    if inner.accept(&msg) {
                        inner.on_recv_ack(ctx, timeout, msg);
                        State::Idle
                    } else {
                        State::Idle.recv(ctx, inner, timeout)
                    }
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
            any => {
                ctx.raise(Event::CanRecv(true));
                any
            }
        }
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
    fn send(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Send is not supported by pull protocol");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
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

    fn subscribe(&mut self, subscription :String) {
        self.subscriptions.insert(subscription.into_bytes());
    }

    fn unsubscribe(&mut self, subscription :String) {
        self.subscriptions.remove(&subscription.into_bytes());
    }

    fn accept(&self, msg: &Message) -> bool {
        let payload = msg.get_body();
        
        self.subscriptions.iter().any(|s| payload.starts_with(s))
    }
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }
}
