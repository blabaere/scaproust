// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;
use std::fmt;
use std::sync::mpsc::Sender;

use super::network::Network;
use message::Message;

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

// Maybe there should be 'local' endpoints (results of a bind) 
// and 'remote' endpoints (results of a connect)
// 
// Protocols care only about remote endpoints because that's where send/recv operations are.
//
// Local endpoints can only be operated on from the facade, and only for closing
// The event loop can notify an error, handled by removing it and scheduling a retry.
// So while they are similar from the facade point of view, 
// the core should really have two different concepts.
//
// Idea: put an additional bool in the EndpointId struct ?
// And a method to return that bool
// Or use even and odd values to test the distinction ?

pub struct Endpoint {
    id: EndpointId,
    url: Option<String>
    // priorities should go there too
    // therefore, endpoint should have open, close, send and recv methods
}

impl Endpoint {
    pub fn new_created(id: EndpointId, url: String) -> Endpoint {
        Endpoint {
            id: id,
            url: Some(url)
        }
    }

    pub fn new_accepted(id: EndpointId) -> Endpoint {
        Endpoint {
            id: id,
            url: None
        }
    }

    pub fn open(&self, network: &mut Network) {
        network.open(self.id)
    }
    pub fn close(&self, network: &mut Network) {
        network.close(self.id)
    }
    pub fn send(&self, network: &mut Network, msg: Rc<Message>) {
        network.send(self.id, msg)
    }
    pub fn recv(&self, network: &mut Network) {
        network.recv(self.id)
    }
}
