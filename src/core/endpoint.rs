// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::io;
use std::fmt;
use std::sync::mpsc::Sender;

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
}

pub struct EndpointCollection {
    id_sequence: usize,
    endpoints: HashMap<EndpointId, Endpoint>
}

impl EndpointCollection {
    pub fn new() -> EndpointCollection {
        EndpointCollection {
            id_sequence: 0,
            endpoints: HashMap::new()
        }
    }

    pub fn reserve_id(&mut self) -> EndpointId {
        let id = EndpointId(self.id_sequence);

        self.id_sequence += 1;

        id
    }

}
