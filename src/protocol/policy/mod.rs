// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use EventLoop;
use event_loop_msg::{ SocketNotify };
use transport::pipe::Pipe;
use global;

pub mod priolist;

pub mod with_fair_queue;
pub mod with_load_balancing;
pub mod with_unicast_send;
pub mod with_unicast_recv;
pub mod with_broadcast;

pub use self::priolist::PrioList;

pub use self::with_fair_queue::WithFairQueue;
pub use self::with_load_balancing::WithLoadBalancing;
pub use self::with_unicast_send::WithUnicastSend;
pub use self::with_unicast_recv::WithUnicastRecv;
pub use self::with_broadcast::WithBroadcast;

pub type Timeout = Option<mio::Timeout>;

pub fn clear_timeout(event_loop: &mut EventLoop, handle: Option<mio::Timeout>) {
    if let Some(timeout) = handle {
        event_loop.clear_timeout(&timeout);
    }
}

pub trait WithNotify {
    fn get_notify_sender(&self) -> &Sender<SocketNotify>;

    fn send_notify(&self, evt: SocketNotify) {
        let send_res = self.get_notify_sender().send(evt);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
        }
    }
}

pub trait WithPipes : WithNotify {
    fn get_pipes(&self) -> &HashMap<mio::Token, Pipe>;
    fn get_pipes_mut(&mut self) -> &mut HashMap<mio::Token, Pipe>;

    fn get_pipe(&self, tok: &mio::Token) -> Option<&Pipe> {
        self.get_pipes().get(tok)
    }

    fn get_pipe_mut(&mut self, tok: &mio::Token) -> Option<&mut Pipe> {
        self.get_pipes_mut().get_mut(tok)
    }

    fn destroy_pipes(&mut self, event_loop: &mut EventLoop) {
        let _: Vec<_> = self.get_pipes_mut().drain().map(|(_, mut p)| p.close(event_loop)).collect();
    }

    fn insert_into_pipes(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.get_pipes_mut().insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(global::invalid_data_io_error("A pipe has already been added with that token"))
        }
    }
}

pub trait WithoutRecv : WithNotify{

    fn recv(&mut self) {
        let err = global::other_io_error("recv not supported by protocol");
        let ntf = SocketNotify::MsgNotRecv(err);

        self.send_notify(ntf);
    }
}

pub trait WithoutSend : WithNotify{

    fn send(&self) {
        let err = global::other_io_error("send not supported by protocol");
        let ntf = SocketNotify::MsgNotSent(err);

        self.send_notify(ntf);
    }
}

pub trait WithBacktrace {

    fn get_backtrace(&self) -> &Vec<u8>;
    fn get_backtrace_mut(&mut self) -> &mut Vec<u8>;

    fn backtrace(&self) -> &[u8] {
        &self.get_backtrace()
    }

    fn set_backtrace(&mut self, backtrace: &[u8]) {
        self.get_backtrace_mut().clear();
        self.get_backtrace_mut().extend_from_slice(backtrace);
    }

    fn clear_backtrace(&mut self) {
        self.get_backtrace_mut().clear();
    }
}