// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::fmt;
use std::io;

use core::network::Network;

pub enum Event {
    CanSend,
    CanRecv
}

pub enum Schedulable {
    Reconnect,
    Rebind,
    SendTimeout,
    RecvTimeout
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Scheduled(usize);

pub trait Scheduler {
    fn schedule(&mut self, schedulable: Schedulable) -> io::Result<Scheduled>;
    fn cancel(&mut self, scheduled: Scheduled);
}

pub trait Context /* : Network + Scheduler */{
    fn raise(&mut self, evt: Event);
}

impl fmt::Debug for Scheduled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for Scheduled {
    fn from(value: usize) -> Scheduled {
        Scheduled(value)
    }
}

impl Into<usize> for Scheduled {
    fn into(self) -> usize {
        self.0
    }
}

impl<'x> Into<usize> for &'x Scheduled {
    fn into(self) -> usize {
        self.0
    }
}
