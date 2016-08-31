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

use core::session;
use core::context::Schedulable;
use reactor::{EventLoop, Handler};
use super::{Signal, Request};
use super::bus::EventLoopBus;

const CHANNEL_TOKEN: Token = Token(::std::usize::MAX - 1);
const BUS_TOKEN: Token     = Token(::std::usize::MAX - 2);
const TIMER_TOKEN: Token   = Token(::std::usize::MAX - 3);

pub struct Dispatcher<P:Processor> {
    channel: Receiver<Request>,
    bus: EventLoopBus<Signal>,
    timer: Timer<Schedulable>,
    processor: P
}

/*pub fn run_event_loop<P:Controller>(
    request_rx: Receiver<Request>,
    reply_tx: Sender<session::Reply>) -> io::Result<()> {


    try!(poll.register(&request_rx, CHANNEL_TOKEN, Ready::readable(), PollOpt::edge()));
    try!(poll.register(&bus, BUS_TOKEN, Ready::readable(), PollOpt::edge()));
    try!(poll.register(&timer, TIMER_TOKEN, Ready::readable(), PollOpt::edge()));

    let mut dispatcher = Dispatcher {
        channel: request_rx,
        bus: bus,
        timer: timer,
        controller: Controller
    };
    let mut event_loop = EventLoop::new(poll);

    event_loop.run(&mut dispatcher)
}*/

pub struct ProcessorContext<'a, 'b> {
    poll: &'a mut Poll,
    bus: &'b mut EventLoopBus<Signal>,
    timer: &'b mut Timer<Schedulable>
}

pub trait Processor : Sized {
    fn process_request(&mut self, ctx: &mut ProcessorContext, req: Request);
    fn process_signal(&mut self, ctx: &mut ProcessorContext, signal: Signal);
    fn process_timeout(&mut self, ctx: &mut ProcessorContext, timeout: Schedulable);
    fn process_io(&mut self, ctx: &mut ProcessorContext, token: Token, events: Ready);
}

impl<P:Processor> Dispatcher<P> {
    pub fn new(request_rx: Receiver<Request>, p: P) -> Dispatcher<P> {
        Dispatcher {
            channel: request_rx,
            bus: EventLoopBus::new(),
            timer: Timer::default(),
            processor: p
        }
    }
    pub fn run(&mut self) -> io::Result<()> {
        let mut poll = try!(Poll::new());
        let mut event_loop = EventLoop::new(poll);

        event_loop.run(self)
    }
    fn process_channel(&mut self, poll: &mut Poll) {
        /*for _ in 0..self.config.messages_per_tick {
            match self.channel.try_recv() {
                Ok(req) => self.dispatcher.process_request(poll, req),
                _ => break,
            }
        }*/
        let mut ctx = ProcessorContext {
            poll: poll,
            bus: &mut self.bus,
            timer: &mut self.timer
        };

        while let Ok(req) = self.channel.try_recv() {
            self.processor.process_request(&mut ctx, req);
        }
    }

    fn process_bus(&mut self, poll: &mut Poll) {
        while let Some(signal) = self.bus.recv() {
            self.process_bus_signal(poll, signal);
        }
    }
    #[inline]
    fn process_bus_signal(&mut self, poll: &mut Poll, signal: Signal) {
        let mut ctx = ProcessorContext {
            poll: poll,
            bus: &mut self.bus,
            timer: &mut self.timer
        };
        self.processor.process_signal(&mut ctx, signal);
    }

    fn process_timer(&mut self, poll: &mut Poll) {
        while let Some(timeout) = self.timer.poll() {
            self.process_timer_tick(poll, timeout);
        }
    }
    #[inline]
    fn process_timer_tick(&mut self, poll: &mut Poll, timeout: Schedulable) {
        let mut ctx = ProcessorContext {
            poll: poll,
            bus: &mut self.bus,
            timer: &mut self.timer
        };
        self.processor.process_timeout(&mut ctx, timeout);
    }
    fn process_io(&mut self, poll: &mut Poll, token: Token, events: Ready) {
        let mut ctx = ProcessorContext {
            poll: poll,
            bus: &mut self.bus,
            timer: &mut self.timer
        };
        self.processor.process_io(&mut ctx, token, events);
    }
}

impl<P:Processor> Handler for Dispatcher<P> {
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

        self.process_io(poll, token, events)
    }
}

#[cfg(test)]
mod tests {
    use ctrl::{Request, Signal};
    use core::context::Schedulable;
    use mio::{Token, Evented, Ready};
    use mio::channel::channel;
    use std::collections::HashMap;

    use super::*;

    fn run_event_loop() {
        let controller = Controller {
            notifications: HashMap::new(),
            items: HashMap::new()
        };
        let(tx, rx) = channel();
        let mut dispatcher = Dispatcher::new(rx, controller);
        let _ = dispatcher.run();
    }

    struct Controller {
        notifications: HashMap<Token, usize>,
        items: HashMap<Token, Item>
    }

    impl Processor for Controller {
        fn process_request(&mut self, ctx: &mut ProcessorContext, req: Request) {}
        fn process_signal(&mut self, ctx: &mut ProcessorContext, signal: Signal) {}
        fn process_timeout(&mut self, ctx: &mut ProcessorContext, timeout: Schedulable) {}
        fn process_io<'x, 'y>(&mut self, ctx: &mut ProcessorContext<'x, 'y>, token: Token, events: Ready) {
            let mut item = self.items.get_mut(&token).unwrap();
            let mut item_ctx = ItemContext {
                parent: ctx,
                state: &mut self.notifications
            };

            item.work(&mut item_ctx);
        }
    }

    trait Registrar {
        fn register(&mut self, io: &Evented);
    }

    struct Item {
        x: char
    }

    impl Item {
        fn work(&mut self, _: &mut Registrar) {
            self.x = 'a';
            // just pretend to be mutating something through stack references
        }
    }

    struct ItemContext<'a, 'b, 'x : 'a, 'y : 'a> {
        parent: &'a mut ProcessorContext<'x, 'y>,
        state: &'b mut HashMap<Token, usize>
    }

    impl<'a, 'b, 'x, 'y> Registrar for ItemContext<'a, 'b, 'x, 'y> {
        fn register(&mut self, io: &Evented) {

        }
    }

}