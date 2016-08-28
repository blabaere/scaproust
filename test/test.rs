// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

extern crate scaproust;

use std::rc::Rc;
use std::sync::mpsc;

use scaproust::*;
use scaproust::core::context::{Context, Scheduled};
use scaproust::core::socket::{Protocol, Reply};
use scaproust::core::{Message, EndpointId};
use scaproust::core::endpoint::Pipe;

#[test]
fn can_create_socket() {
    let mut session = SessionBuilder::build().unwrap();
    let mut socket = session.create_socket::<Push>().unwrap();
    let ep = socket.connect("tcp://127.0.0.1:5454").unwrap();

    let _ = ep.close();
}

pub struct Push {
    sender: mpsc::Sender<Reply>,
    pipe_id: Option<EndpointId>,
    pipe: Option<Pipe>
}
impl Protocol for Push {
    fn id(&self) -> u16 { (5 * 16) }
    fn peer_id(&self) -> u16 { (5 * 16) + 1 }
    fn add_pipe(&mut self, _: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.pipe_id = Some(eid);
        self.pipe = Some(pipe);
    }
    fn remove_pipe(&mut self, _: &mut Context, _: EndpointId) -> Option<Pipe> {
        let (_, pipe) = (self.pipe_id.take(), self.pipe.take());

        pipe
    }
    fn send(&mut self, ctx: &mut Context, msg: Message, _: Option<Scheduled>) {
        self.pipe_id.map(|eid| ctx.send(eid, Rc::new(msg)));
    }
    fn on_send_ack(&mut self, _: &mut Context, _: EndpointId) {}
    fn on_send_timeout(&mut self, _: &mut Context) {}
    fn recv(&mut self, _: &mut Context, _: Option<Scheduled>) {}
    fn on_recv_ack(&mut self, _: &mut Context, _: EndpointId, _: Message) {}
    fn on_recv_timeout(&mut self, _: &mut Context) {}
}

impl From<mpsc::Sender<Reply>> for Push {
    fn from(tx: mpsc::Sender<Reply>) -> Push {
        Push {
            sender: tx,
            pipe_id: None,
            pipe: None
        }
    }
}
