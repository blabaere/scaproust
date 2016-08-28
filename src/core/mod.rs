// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod network;
pub mod context;
pub mod config;
pub mod socket;
pub mod session;
pub mod endpoint;

use std::fmt;

/*****************************************************************************/
/*                                                                           */
/* EndpointId                                                                */
/*                                                                           */
/*****************************************************************************/

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

/*****************************************************************************/
/*                                                                           */
/* EndpointSpec                                                              */
/*                                                                           */
/*****************************************************************************/

pub struct EndpointSpec {
    pub id: EndpointId,
    pub url: String,
    pub send_priority: u8,
    pub recv_priority: u8
}

/*****************************************************************************/
/*                                                                           */
/* SocketId                                                                  */
/*                                                                           */
/*****************************************************************************/

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct SocketId(usize);

impl fmt::Debug for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for SocketId {
    fn from(value: usize) -> SocketId {
        SocketId(value)
    }
}

/*****************************************************************************/
/*                                                                           */
/* Message                                                                   */
/*                                                                           */
/*****************************************************************************/

pub struct Message {
    header: Vec<u8>,
    body: Vec<u8>
}

impl Message {
    pub fn new() -> Message {
        Message {
            header: Vec::new(),
            body: Vec::new()
        }
    }

    pub fn from_body(body: Vec<u8>) -> Message {
        Message {
            header: Vec::new(),
            body: body
        }
    }

    pub fn from_header_and_body(header: Vec<u8>, body: Vec<u8>) -> Message {
        Message {
            header: header,
            body: body
        }
    }

    pub fn len(&self) -> usize {
        self.header.len() + self.body.len()
    }

    pub fn get_header(&self) -> &[u8] {
        &self.header
    }

    pub fn get_body(&self) -> &[u8] {
        &self.body
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        self.body
    }
}
