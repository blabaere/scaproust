// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::fmt;
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

pub trait Context : Scheduler + fmt::Debug {
    fn poll(&mut self, sid: SocketId);
}

pub struct Probe {
    reply_sender: Sender<Reply>,
    poll_opts: Vec<PollReq>,
    timeout: Option<Scheduled>
}

impl Probe {
    pub fn new(reply_tx: Sender<Reply>, poll_opts: Vec<PollReq>) -> Probe {
        Probe {
            reply_sender: reply_tx,
            poll_opts: poll_opts,
            timeout: None
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn poll(&mut self, ctx: &mut Context, delay: Duration) {
        #[cfg(debug_assertions)] debug!("[{:?}] poll", ctx);
        let task = Schedulable::PollTimeout;

        match ctx.schedule(task, delay) {
            Ok(timeout) => self.timeout = Some(timeout),
            Err(e) => self.send_reply(Reply::Err(e))
        }
    }

    pub fn on_poll_timeout(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] on_poll_timeout", ctx);
        self.send_reply(Reply::Poll);
    }

    pub fn on_socket_can_recv(&mut self, sid: SocketId, can_recv: bool) {
        #[cfg(debug_assertions)] debug!("Probe on_socket_can_recv {:?} {}", sid, can_recv);
    }

    pub fn on_socket_can_send(&mut self, sid: SocketId, can_send: bool) {
        #[cfg(debug_assertions)] debug!("Probe on_socket_can_send {:?} {}", sid, can_send);
    }
}