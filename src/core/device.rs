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
    Check(bool, bool),
    Closed
}

pub trait Context {
    fn poll(&mut self, sid: SocketId);
}

pub struct Device {
    reply_sender: Sender<Reply>,
    left: SocketId,
    right: SocketId,
    left_recv: bool,
    right_recv: bool,
    waiting: bool
}

impl Device {
    pub fn new(reply_tx: Sender<Reply>, l: SocketId, r: SocketId) -> Device {
        Device {
            reply_sender: reply_tx,
            left: l,
            right: r,
            left_recv: false,
            right_recv: false,
            waiting: false
        }
    }

    pub fn check(&mut self, ctx: &mut Context) {
        if self.left_recv | self.right_recv {
            self.send_check_reply();
        } else {
            ctx.poll(self.left);
            ctx.poll(self.right);

            self.waiting = true;
        }
    }

    pub fn on_socket_can_recv(&mut self, sid: SocketId, can_recv: bool) {
        if sid == self.left {
            self.left_recv = can_recv;
        } else if sid == self.right {
            self.right_recv = can_recv;
        }

        if can_recv && self.waiting {
            self.send_check_reply();
        }
    }

    fn send_check_reply(&mut self) {
        let reply = Reply::Check(self.left_recv, self.right_recv);

        self.send_reply(reply);
        self.waiting = false;
    }

    fn send_reply(&mut self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn get_left_id(&self) -> &SocketId {
        &self.left
    }

    pub fn get_right_id(&self) -> &SocketId {
        &self.right
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.send_reply(Reply::Closed)
    }
}
