// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

extern crate scaproust;

use std::rc::Rc;
use std::sync::mpsc;

use scaproust::*;

#[test]
fn can_create_socket() {
    let mut session = SessionBuilder::build().unwrap();
    let mut socket = session.create_socket::<Push>().unwrap();
    let ep = socket.connect("tcp://127.0.0.1:5454").unwrap();
}

pub struct Push {
    sender: mpsc::Sender<SocketReply>,
    pipe_id: Option<EndpointId>,
    pipe: Option<Pipe>
}
impl Protocol for Push {
    fn id(&self) -> u16 { (5 * 16) }
    fn peer_id(&self) -> u16 { (5 * 16) + 1 }
    fn add_pipe(&mut self, network: &mut Network, eid: EndpointId, pipe: Pipe) {
        self.pipe_id = Some(eid);
        self.pipe = Some(pipe);
    }
    fn remove_pipe(&mut self, network: &mut Network, eid: EndpointId) -> Option<Pipe> {
        let (_, pipe) = (self.pipe_id.take(), self.pipe.take());

        pipe
    }
    fn send(&mut self, network: &mut Network, msg: Message) {
        self.pipe_id.map(|eid| network.send(eid, Rc::new(msg)));
    }
    fn on_send_ack(&mut self, network: &mut Network, eid: EndpointId) {}
    fn recv(&mut self, network: &mut Network) {

    }
}

impl From<mpsc::Sender<SocketReply>> for Push {
    fn from(tx: mpsc::Sender<SocketReply>) -> Push {
        Push {
            sender: tx,
            pipe_id: None,
            pipe: None
        }
    }
}
