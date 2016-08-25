// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::fmt;

use super::network::Network;
use super::message::Message;

pub enum Request {
    Close
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct EndpointId(usize);

impl fmt::Debug for EndpointId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for EndpointId {
    fn from(value: usize) -> EndpointId {
        EndpointId(value)
    }
}

impl Into<usize> for EndpointId {
    fn into(self) -> usize {
        self.0
    }
}

impl<'x> Into<usize> for &'x EndpointId {
    fn into(self) -> usize {
        self.0
    }
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

    fn open(&self, network: &mut Network) {
        network.open(self.id)
    }
    fn close(&self, network: &mut Network) {
        network.close(self.id)
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
        self.0.open(network)
    }
    pub fn close(&self, network: &mut Network) {
        self.0.close(network)
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
        self.0.open(network)
    }
    pub fn close(&self, network: &mut Network) {
        self.0.close(network)
    }
}
