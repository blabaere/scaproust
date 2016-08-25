// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use core::{socket, endpoint};
use transport::{PipeCmd, PipeEvt, AcceptorCmd, AcceptorEvt};

/// Commands and events flowing between the controller and transport components.
pub enum Signal {
    PipeCmd(socket::SocketId, endpoint::EndpointId, PipeCmd),
    PipeEvt(socket::SocketId, endpoint::EndpointId, PipeEvt),
    AcceptorCmd(socket::SocketId, endpoint::EndpointId, AcceptorCmd),
    AcceptorEvt(socket::SocketId, endpoint::EndpointId, AcceptorEvt)
}
