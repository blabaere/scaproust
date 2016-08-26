// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use core::socket::SocketId;
use core::endpoint::EndpointId;
use transport::pipe;
use transport::acceptor;

/// Commands and events flowing between the controller and transport components.
/// The controller sends commands, the transport sends events.
pub enum Signal {
    PipeCmd(SocketId, EndpointId, pipe::Command),
    PipeEvt(SocketId, EndpointId, pipe::Event),
    AcceptorCmd(SocketId, EndpointId, acceptor::Command),
    AcceptorEvt(SocketId, EndpointId, acceptor::Event)
}
