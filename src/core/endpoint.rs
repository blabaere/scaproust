// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use super::{EndpointId, Message, EndpointSpec};
use super::context::Context;

pub enum Request {
    Close(bool)
}

pub struct Endpoint {
    id: EndpointId,
    url: Option<String>,
    send_priority: u8,
    recv_priority: u8
}

pub struct Pipe(Endpoint);
pub struct Acceptor(Endpoint);

impl Endpoint {
    fn new_created(id: EndpointId, url: String, send_prio: u8, recv_prio: u8) -> Endpoint {
        Endpoint {
            id: id,
            url: Some(url),
            send_priority: send_prio,
            recv_priority: recv_prio,
        }
    }

    fn new_accepted(id: EndpointId, send_prio: u8, recv_prio: u8) -> Endpoint {
        Endpoint {
            id: id,
            url: None,
            send_priority: send_prio,
            recv_priority: recv_prio,
        }
    }

    fn open(&self, network: &mut Context, remote: bool) {
        network.open(self.id, remote)
    }
    fn send(&self, network: &mut Context, msg: Rc<Message>) {
        network.send(self.id, msg)
    }
    fn recv(&self, network: &mut Context) {
        network.recv(self.id)
    }
    fn close(mut self, network: &mut Context, remote: bool) -> Option<EndpointSpec> {
        network.close(self.id, remote);
        
        self.url.take().map(|url| EndpointSpec {
            id: self.id,
            url: url,
            send_priority: self.send_priority,
            recv_priority: self.recv_priority
        })
    }
}

impl From<EndpointSpec> for Pipe {
    fn from(spec: EndpointSpec) -> Pipe {
        Pipe::new_connected(spec.id, spec.url, spec.send_priority, spec.recv_priority)
    }
}

impl Pipe {
    pub fn new_connected(id: EndpointId, url: String, send_prio: u8, recv_prio: u8) -> Pipe {
        Pipe(Endpoint::new_created(id, url, send_prio, recv_prio))
    }

    pub fn new_accepted(id: EndpointId, send_prio: u8, recv_prio: u8) -> Pipe {
        Pipe(Endpoint::new_accepted(id, send_prio, recv_prio))
    }

    pub fn open(&self, network: &mut Context) {
        println!("core::Pipe::open {:?}", self.0.id);
        self.0.open(network, true)
    }
    pub fn send(&self, network: &mut Context, msg: Rc<Message>) {
        println!("core::Pipe::send {:?}", self.0.id);
        self.0.send(network, msg)
    }
    pub fn recv(&self, network: &mut Context) {
        println!("core::Pipe::recv {:?}", self.0.id);
        self.0.recv(network)
    }
    pub fn close(mut self, network: &mut Context) -> Option<EndpointSpec> {
        println!("core::Pipe::close {:?}", self.0.id);
        self.0.close(network, true)
    }
    pub fn get_send_priority(&self) -> u8 {
        self.0.send_priority
    }
    pub fn get_recv_priority(&self) -> u8 {
        self.0.recv_priority
    }
}

impl Acceptor {
    pub fn new(id: EndpointId, url: String, send_prio: u8, recv_prio: u8) -> Acceptor {
        Acceptor(Endpoint::new_created(id, url, send_prio, recv_prio))
    }
    pub fn open(&self, network: &mut Context) {
        self.0.open(network, false)
    }
    pub fn close(mut self, network: &mut Context) -> Option<EndpointSpec> {
        self.0.close(network, false)
    }
    pub fn get_send_priority(&self) -> u8 {
        self.0.send_priority
    }
    pub fn get_recv_priority(&self) -> u8 {
        self.0.recv_priority
    }
}

impl From<EndpointSpec> for Acceptor {
    fn from(spec: EndpointSpec) -> Acceptor {
        Acceptor::new(spec.id, spec.url, spec.send_priority, spec.recv_priority)
    }
}
