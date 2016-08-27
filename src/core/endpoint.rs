// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use super::EndpointId;
use super::network::Network;
use super::message::Message;

pub enum Request {
    Close(bool)
}

pub struct Endpoint {
    id: EndpointId,
    url: Option<String>
    // priorities should go there too
}

pub struct Pipe(Endpoint);
pub struct Acceptor(Endpoint);

impl Endpoint {
    fn new_created(id: EndpointId, url: String) -> Endpoint {
        Endpoint {
            id: id,
            url: Some(url)
        }
    }

    fn new_accepted(id: EndpointId) -> Endpoint {
        Endpoint {
            id: id,
            url: None
        }
    }

    fn open(&self, network: &mut Network, remote: bool) {
        network.open(self.id, remote)
    }
    fn close(&self, network: &mut Network, remote: bool) {
        network.close(self.id, remote)
    }
    fn send(&self, network: &mut Network, msg: Rc<Message>) {
        network.send(self.id, msg)
    }
    fn recv(&self, network: &mut Network) {
        network.recv(self.id)
    }
}

impl Pipe {
    pub fn new_created(id: EndpointId, url: String) -> Pipe {
        Pipe(Endpoint::new_created(id, url))
    }

    pub fn new_accepted(id: EndpointId) -> Pipe {
        Pipe(Endpoint::new_accepted(id))
    }

    pub fn open(&self, network: &mut Network) {
        self.0.open(network, true)
    }
    pub fn close(&self, network: &mut Network) {
        self.0.close(network, true)
    }
    pub fn send(&self, network: &mut Network, msg: Rc<Message>) {
        self.0.send(network, msg)
    }
    pub fn recv(&self, network: &mut Network) {
        self.0.recv(network)
    }
}

impl Acceptor {
    pub fn new(id: EndpointId, url: String) -> Acceptor {
        Acceptor(Endpoint::new_created(id, url))
    }
    pub fn open(&self, network: &mut Network) {
        self.0.open(network, false)
    }
    pub fn close(&self, network: &mut Network) {
        self.0.close(network, false)
    }
}
