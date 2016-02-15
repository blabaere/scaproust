// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;

use global::*;
use event_loop_msg::*;

pub struct Probe {
    left_socket_id: SocketId,
    right_socket_id: SocketId,
    evt_sender: Sender<ProbeNotify>
}

impl Probe {
    pub fn new(l: SocketId, r: SocketId, evt_tx: Sender<ProbeNotify>) -> Probe {
        Probe {
            left_socket_id: l,
            right_socket_id: r,
            evt_sender: evt_tx
        }
    }

    pub fn on_socket_readable(&mut self, id: SocketId) {
        if id == self.left_socket_id {
            self.send_notify(ProbeNotify::Ok(true, false))
        } else if id == self.right_socket_id {
            self.send_notify(ProbeNotify::Ok(false, true))
        }
    }

    fn send_notify(&self, ntf: ProbeNotify) {
        let send_res = self.evt_sender.send(ntf);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
        }
    }
}