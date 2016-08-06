// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use facade::Sender;
use core::socket::{Request, Reply};
use ctrl::EventLoopSignal;
use util::*;

use mio;

pub type SignalSender = Rc<mio::Sender<EventLoopSignal>>;
pub type ReplyReceiver = mpsc::Receiver<Reply>;

pub struct Socket {
    request_sender: SignalSender,
    reply_receiver: ReplyReceiver
}

impl Socket {
    pub fn new(request_tx: SignalSender, reply_rx: ReplyReceiver) -> Socket {
        Socket {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }
}