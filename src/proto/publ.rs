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
use core::context::Context;
use super::{Timeout, PUB, SUB};
use io_error::*;

pub struct Pub {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    bc: HashSet<EndpointId>
}

/*****************************************************************************/
/*                                                                           */
/* Pub                                                                       */
/*                                                                           */
/*****************************************************************************/

impl From<Sender<Reply>> for Pub {
    fn from(tx: Sender<Reply>) -> Pub {
        Pub {
            reply_tx: tx,
            pipes: HashMap::new(),
            bc: HashSet::new()
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

impl Protocol for Pub {
    fn id(&self)      -> u16 { PUB }
    fn peer_id(&self) -> u16 { SUB }

    fn add_pipe(&mut self, _: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, _: &mut Context, eid: EndpointId) -> Option<Pipe> {
        self.bc.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Timeout) {
        let msg = Rc::new(msg);

        for id in self.bc.drain() {
            self.pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
        }

        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_send_ack(&mut self, _: &mut Context, _: EndpointId) {
    }
    fn on_send_timeout(&mut self, _: &mut Context) {
    }
    fn on_send_ready(&mut self, _: &mut Context, eid: EndpointId) {
        self.bc.insert(eid);
    }
    fn recv(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Recv is not supported by pub protocol");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_ack(&mut self, _: &mut Context, _: EndpointId, _: Message) {
    }
    fn on_recv_timeout(&mut self, _: &mut Context) {
    }
    fn on_recv_ready(&mut self, _: &mut Context, _: EndpointId) {
    }
}
