// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;

use mio;

use protocol::policy::{ Timeout, clear_timeout };
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;
use super::WithPipes;

pub trait WithBroadcast : WithPipes {
    fn send_all(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Timeout) {
        self.broadcast(event_loop, msg, &mut |_| true);
        self.send_notify(SocketNotify::MsgSent);
        clear_timeout(event_loop, timeout);
    }

    fn send_all_but(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Timeout, except: mio::Token) {
        self.broadcast(event_loop, msg, &mut |&tok| tok != except);
        self.send_notify(SocketNotify::MsgSent);
        clear_timeout(event_loop, timeout);
    }

    fn broadcast<P>(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, predicate: &mut P) 
    where P : FnMut(&mio::Token) -> bool {
        let pipes = self.
            get_pipes_mut().
            iter_mut().
            filter(|&(ref tok ,ref p)| predicate(tok) && p.can_send());
            
        for (_, pipe) in pipes {
            pipe.send(event_loop, msg.clone());
        }
    }

    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(&tok).map(|p| p.resync_readiness(event_loop));
    }
}
