// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use super::socket::SocketId;
use super::endpoint::EndpointId;

pub trait Network {
    // this is not going to work with reconnect
    // the endpoint id should be selected by the socket
    // and then passed to the network functions
    fn connect(&self, socket_id: SocketId, url: &str) -> io::Result<EndpointId>;
    fn bind(&self, socket_id: SocketId, url: &str) -> io::Result<EndpointId>;
}