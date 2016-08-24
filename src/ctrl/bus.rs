// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::cell::RefCell;
use std::io::Result;
use std::collections::VecDeque;

use mio::{Registration, SetReadiness, Evented, Poll, Token, EventSet, PollOpt};

use io_error::*;

pub struct EventLoopBus<T> {
    queue: VecDeque<T>,
    registration: RefCell<Option<Registration>>,
    readiness: RefCell<Option<SetReadiness>>
}

impl<T> EventLoopBus<T> {
    pub fn new() -> EventLoopBus<T> {
        EventLoopBus {
            queue: VecDeque::new(),
            registration: RefCell::new(None),
            readiness: RefCell::new(None)
        }
    }

    pub fn send(&mut self, t: T) {
        if self.queue.len() == 0 {
            self.set_readiness(EventSet::readable());
        }

        self.queue.push_back(t)
    }

    pub fn recv(&mut self) -> Option<T> {
        if self.queue.len() == 1 {
            self.set_readiness(EventSet::none());
        }

        self.queue.pop_front()
    }

    fn set_readiness(&mut self, events: EventSet) {
        if let Some(ref readiness) = *self.readiness.borrow_mut() {
            let _ = readiness.set_readiness(events);
        }
    }
}

impl<T> Evented for EventLoopBus<T> {
    fn register(&self, poll: &Poll, token: Token, interest: EventSet, opts: PollOpt) -> Result<()> {
        if self.registration.borrow().is_some() {
            return Err(other_io_error("bus already registered"));
        }

        let (registration, readiness) = Registration::new(poll, token, interest, opts);

        if self.queue.len() > 0 {
            let _ = readiness.set_readiness(EventSet::readable());
        }

        //let registration_ref = self.registration.borrow_mut();
        //let readiness_ref = self.readiness.borrow_mut();

        *self.registration.borrow_mut() = Some(registration);
        *self.readiness.borrow_mut() = Some(readiness);

        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: EventSet, opts: PollOpt) -> Result<()> {
        match *self.registration.borrow_mut() {
            Some(ref registration) => registration.update(poll, token, interest, opts),
            None => Err(other_io_error("bus not registered")),
        }
    }

    fn deregister(&self, poll: &Poll) -> Result<()> {
        match *self.registration.borrow_mut() {
            Some(ref registration) => registration.deregister(poll),
            None => Err(other_io_error("bus not registered")),
        }
    }

}

/*
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let bus = EventLoopBus::new();
    let bus_ref = Rc::new(RefCell::new(bus));
    let sender = Sender { bus: bus_ref };
    let receiver = Receiver { bus: bus_ref, registration: None };

    (sender, receiver)
}

pub struct Sender<T> {
    bus: Rc<RefCell<EventLoopBus<T>>>
}

pub struct Receiver<T> {
    bus: Rc<RefCell<EventLoopBus<T>>>,
    registration: Option<Registration>
}

struct EventLoopBus<T> {
    queue: Vec<T>,
    readiness: Option<SetReadiness>
}

impl Sender {
    pub fn send(&self, t T) {
        unimplemented!();
    }
}

impl<T> EventLoopBus<T> {
    fn new() -> EventLoopBus<T> {
        EventLoopBus {
            queue: Vec::new(),
            readiness: None
        }
    }
}

impl<T> Evented for Receiver<T> {
    fn register(&self, poll: &Poll, token: Token, interest: EventSet, opts: PollOpt) -> Result<()> {
        if self.bus.borrow().registration.is_some() {
            return Err(other_io_error("bus already registered"));
        }
        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: EventSet, opts: PollOpt) -> Result<()> {
        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> Result<()> {
        Ok(())
    }

}
*/