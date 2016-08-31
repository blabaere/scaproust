// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;
use std::io;

use mio::{Token, Poll, Ready, PollOpt};
use mio::timer::{Timer};
use mio::channel::{Receiver};

use core::{SocketId, EndpointId, session, socket, endpoint};
use core::context::Schedulable;
use reactor::{EventLoop, Handler};
use super::{Signal, Request};
use super::bus::EventLoopBus;

const CHANNEL_TOKEN: Token = Token(::std::usize::MAX - 1);
const BUS_TOKEN: Token     = Token(::std::usize::MAX - 2);
const TIMER_TOKEN: Token   = Token(::std::usize::MAX - 3);

pub struct Controller {
    channel: Receiver<Request>,
    bus: EventLoopBus<Signal>,
    timer: Timer<Schedulable>,
    dispatcher: Dispatcher
}

pub fn run_event_loop(
    request_rx: Receiver<Request>,
    reply_tx: Sender<session::Reply>) -> io::Result<()> {

    let mut poll = try!(Poll::new());
    let mut bus = EventLoopBus::new();
    let mut timer = Timer::default();

    try!(poll.register(&request_rx, CHANNEL_TOKEN, Ready::readable(), PollOpt::edge()));
    try!(poll.register(&bus, BUS_TOKEN, Ready::readable(), PollOpt::edge()));
    try!(poll.register(&timer, TIMER_TOKEN, Ready::readable(), PollOpt::edge()));

    let mut controller = Controller {
        channel: request_rx,
        bus: bus,
        timer: timer,
        dispatcher: Dispatcher
    };
    let mut event_loop = EventLoop::new(poll);

    event_loop.run(&mut controller)
}

impl Handler for Controller {
    fn process(&mut self, poll: &mut Poll, token: Token, events: Ready) {
        if token == CHANNEL_TOKEN {
            return self.process_channel(poll);
        }
        if token == BUS_TOKEN {
            return self.process_bus(poll);
        }
        if token == TIMER_TOKEN {
            return self.process_timer(poll);
        }

        self.dispatcher.process_io(poll, token, events)
    }
}

impl Controller {
    fn process_channel(&mut self, poll: &mut Poll) {
        /*for _ in 0..self.config.messages_per_tick {
            match self.channel.try_recv() {
                Ok(req) => self.dispatcher.process_request(poll, req),
                _ => break,
            }
        }*/
        while let Ok(req) = self.channel.try_recv() {
            self.dispatcher.process_request(poll, req);
        }
    }
    fn process_bus(&mut self, poll: &mut Poll) {
        while let Some(signal) = self.bus.recv() {
            self.dispatcher.process_signal(poll, signal);
        }
    }
    fn process_timer(&mut self, poll: &mut Poll) {
        while let Some(timeout) = self.timer.poll() {
            self.dispatcher.process_timeout(poll, timeout);
        }
    }
}

struct Dispatcher;

impl Dispatcher {
    fn process_request(&mut self, poll: &mut Poll, req: Request) {

    }
    fn process_signal(&mut self, poll: &mut Poll, signal: Signal) {

    }
    fn process_timeout(&mut self, poll: &mut Poll, timeout: Schedulable) {

    }
    fn process_io(&mut self, poll: &mut Poll, token: Token, events: Ready){}
}