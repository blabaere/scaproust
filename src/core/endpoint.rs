// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

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
