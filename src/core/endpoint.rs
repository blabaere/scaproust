// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use super::{EndpointId, Message, EndpointSpec, EndpointDesc};
use super::context::Context;

pub enum Request {
    Close(bool)
}

pub struct Endpoint {
    id: EndpointId,
    url: Option<String>,
    desc: EndpointDesc
}

pub struct Pipe(Endpoint);
pub struct Acceptor(Endpoint);

impl Endpoint {
    fn new_created(id: EndpointId, url: String, desc: EndpointDesc) -> Endpoint {
        Endpoint {
            id: id,
            url: Some(url),
            desc: desc,
        }
    }

    fn new_accepted(id: EndpointId, desc: EndpointDesc) -> Endpoint {
        Endpoint {
            id: id,
            url: None,
            desc: desc
        }
    }

    fn from_spec(id: EndpointId, spec: EndpointSpec) -> Endpoint {
        Endpoint {
            id: id,
            url: Some(spec.url),
            desc: spec.desc
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

        match self.url.take() {
            Some(url) => Some(EndpointSpec {
                url: url,
                desc: self.desc} ),
            None => None,
        }
    }
    fn get_send_priority(&self) -> u8 {
        self.desc.send_priority
    }
    fn get_recv_priority(&self) -> u8 {
        self.desc.recv_priority
    }
}

impl Pipe {
    pub fn new_connected(id: EndpointId, url: String, desc: EndpointDesc) -> Pipe {
        Pipe(Endpoint::new_created(id, url, desc))
    }

    pub fn new_accepted(id: EndpointId, desc: EndpointDesc) -> Pipe {
        Pipe(Endpoint::new_accepted(id, desc))
    }

    pub fn from_spec(id: EndpointId, spec: EndpointSpec) -> Pipe {
        Pipe(Endpoint::from_spec(id, spec))
    }

    pub fn open(&self, network: &mut Context) {
        self.0.open(network, true)
    }
    pub fn send(&self, network: &mut Context, msg: Rc<Message>) {
        self.0.send(network, msg)
    }
    pub fn recv(&self, network: &mut Context) {
        self.0.recv(network)
    }
    pub fn close(self, network: &mut Context) -> Option<EndpointSpec> {
        self.0.close(network, true)
    }
    pub fn get_send_priority(&self) -> u8 {
        self.0.get_send_priority()
    }
    pub fn get_recv_priority(&self) -> u8 {
        self.0.get_recv_priority()
    }
}

impl Acceptor {
    pub fn new(id: EndpointId, url: String, desc: EndpointDesc) -> Acceptor {
        Acceptor(Endpoint::new_created(id, url, desc))
    }
    pub fn from_spec(id: EndpointId, spec: EndpointSpec) -> Acceptor {
        Acceptor(Endpoint::from_spec(id, spec))
    }
    pub fn open(&self, network: &mut Context) {
        self.0.open(network, false)
    }
    pub fn close(self, network: &mut Context) -> Option<EndpointSpec> {
        self.0.close(network, false)
    }
    pub fn get_send_priority(&self) -> u8 {
        self.0.get_send_priority()
    }
    pub fn get_recv_priority(&self) -> u8 {
        self.0.get_recv_priority()
    }
}
