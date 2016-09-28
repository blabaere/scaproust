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
pub mod device;

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
/* Endpoint                                                                  */
/*                                                                           */
/*****************************************************************************/

pub struct EndpointTmpl {
    pub pids: (u16, u16),
    pub spec: EndpointSpec
}

pub struct EndpointSpec {
    pub url: String,
    pub desc: EndpointDesc
}

pub struct EndpointDesc {
    pub send_priority: u8,
    pub recv_priority: u8,
    pub tcp_no_delay: bool,
    pub recv_max_size: u64
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
/* DeviceId                                                                  */
/*                                                                           */
/*****************************************************************************/

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(usize);

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for DeviceId {
    fn from(value: usize) -> DeviceId {
        DeviceId(value)
    }
}

/*****************************************************************************/
/*                                                                           */
/* Message                                                                   */
/*                                                                           */
/*****************************************************************************/

#[derive(Default)]
pub struct Message {
    pub header: Vec<u8>,
    pub body: Vec<u8>
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

    pub fn split(self) -> (Vec<u8>, Vec<u8>) {
        (self.header, self.body)
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        self.body
    }
}

impl From<Vec<u8>> for Message {
    fn from(value: Vec<u8>) -> Message {
        Message::from_body(value)
    }
}
