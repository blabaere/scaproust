// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::sync::mpsc::Receiver;
use std::time;

use mio;

use global::*;
use event_loop_msg::*;
use Message;

pub struct EndpointFacade {
    socket_id: SocketId,
    endpoint_id: mio::Token,
    cmd_sender: mio::Sender<EventLoopSignal>
}

impl EndpointFacade {
    pub fn new(
        id: SocketId,
        tok: mio::Token, 
        cmd_tx: mio::Sender<EventLoopSignal>) -> EndpointFacade {
        EndpointFacade {
            socket_id: id,
            endpoint_id: tok,
            cmd_sender: cmd_tx
        }
    }

    pub fn shutdown(self) {
        let cmd = SocketCmdSignal::Shutdown(self.endpoint_id);
        let cmd_sig = CmdSignal::Socket(self.socket_id, cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        let _ = self.cmd_sender.send(loop_sig);
    }
}
