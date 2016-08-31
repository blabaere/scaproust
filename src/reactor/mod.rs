// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio::{Poll, Token, Ready, Events};

pub trait Handler {
    fn process(&mut self, poll: &mut Poll, token: Token, events: Ready);
}

pub struct EventLoop {
    events_poller: Poll,
    events: Events,
    running: bool
}

impl EventLoop {
    pub fn new(poll: Poll) -> EventLoop {
        let evts = Events::with_capacity(1024);
        let event_loop = EventLoop {
            events_poller: poll,
            events: evts,
            running: false
        };

        event_loop
    }

    pub fn shutdown(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn run<H: Handler>(&mut self, handler: &mut H) -> io::Result<()> {
        self.running = true;

        while self.running {
            try!(self.run_once(handler));
        }

        Ok(())
    }

    pub fn run_once<H: Handler>(&mut self, handler: &mut H) -> io::Result<()> {
        let event_count = match self.poll_io() {
            Ok(count) => count,
            Err(err) => {
                if err.kind() == io::ErrorKind::Interrupted {
                    0
                } else {
                    return Err(err);
                }
            }
        };

        if event_count > 0 {
            self.process_io(handler);
        }

        Ok(())
    }

    fn poll_io(&mut self) -> io::Result<usize> {
        self.events_poller.poll(&mut self.events, None)
    }

    fn process_io<H: Handler>(&mut self, handler: &mut H) {
        for event in &self.events {
            handler.process(&mut self.events_poller, event.token(), event.kind());
        }
    }
}
