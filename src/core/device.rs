// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;

use super::SocketId;

pub enum Request {
    Check,
    Close
}

pub enum Reply {
    Check(bool, bool)
}

pub struct Device {
    reply_sender: Sender<Reply>,
    left: SocketId,
    right: SocketId,
    left_recv: bool,
    right_recv: bool
}

impl Device {
    pub fn new(reply_tx: Sender<Reply>, l: SocketId, r: SocketId) -> Device {
        Device {
            reply_sender: reply_tx,
            left: l,
            right: r,
            left_recv: false,
            right_recv: false
        }
    }

    pub fn check(&mut self) {
        let _ = self.reply_sender.send(Reply::Check(self.left_recv, self.right_recv));

        self.left_recv = false;
        self.right_recv = false;
    }

    pub fn on_socket_can_recv(&mut self, sid: SocketId) {
        if sid == self.left {
            self.left_recv = true;
        } else if sid == self.right {
            self.right_recv = true;
        }
    }
}