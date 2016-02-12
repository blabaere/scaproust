// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::Sender;

use mio;

use global::*;
use event_loop_msg::*;

use socket::Socket;
use protocol;

use EventLoop;

pub struct Probe {
    left_socket_id: SocketId,
    right_socket_id: SocketId,
    evt_sender: Sender<PollResult>
}

impl Probe {
    pub fn new(l: SocketId, r: SocketId, evt_tx: Sender<PollResult>) -> Probe {
        Probe {
            left_socket_id: l,
            right_socket_id: r,
            evt_sender: evt_tx
        }
    }
}