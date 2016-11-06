// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;
use std::io::{Error, Result};
use std::time::Duration;

use super::{SocketId, PollReq, Scheduled};

pub enum Request {
    Poll(Duration),
    Close
}

pub enum Reply {
    Err(Error),
    Poll
}

pub enum Schedulable {
    PollTimeout
}

pub trait Scheduler {
    fn schedule(&mut self, schedulable: Schedulable, delay: Duration) -> Result<Scheduled>;
    fn cancel(&mut self, scheduled: Scheduled);
}

pub trait Context : Scheduler {
    fn poll(&mut self, sid: SocketId);
}

pub struct Probe {
    reply_sender: Sender<Reply>,
    poll_opts: Vec<PollReq>
}

impl Probe {
    pub fn new(reply_tx: Sender<Reply>, poll_opts: Vec<PollReq>) -> Probe {
        Probe {
            reply_sender: reply_tx,
            poll_opts: poll_opts
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn poll(&mut self, _: &mut Context, timeout: Duration) {
        self.send_reply(Reply::Poll);
    }

    pub fn on_poll_timeout(&mut self, _: &mut Context) {
    }
}