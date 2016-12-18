// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod async;
pub mod tcp;
pub mod ipc;
pub mod endpoint;
pub mod pipe;
pub mod acceptor;

use std::io::Result;

pub struct Destination<'a> {
    pub addr: &'a str,
    pub pids: (u16, u16),
    pub tcp_no_delay: bool,
    pub recv_max_size: u64
}

pub trait Transport {
    fn connect(&self, dest: &Destination) -> Result<Box<pipe::Pipe>>;
    fn bind(&self, dest: &Destination) -> Result<Box<acceptor::Acceptor>>;
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use mio;

    use transport::*;

    pub struct TestPipeContext {
        registrations: Vec<(mio::Ready, mio::PollOpt)>,
        reregistrations: Vec<(mio::Ready, mio::PollOpt)>,
        deregistrations: usize,
        raised_events: Vec<pipe::Event>
    }

    impl TestPipeContext {
        pub fn new() -> TestPipeContext {
            TestPipeContext {
                registrations: Vec::new(),
                reregistrations: Vec::new(),
                deregistrations: 0,
                raised_events: Vec::new()
            }
        }
        pub fn get_registrations(&self) -> &[(mio::Ready, mio::PollOpt)] { &self.registrations }
        pub fn get_reregistrations(&self) -> &[(mio::Ready, mio::PollOpt)] { &self.reregistrations }
        pub fn get_deregistrations(&self) -> usize { self.deregistrations }
        pub fn get_raised_events(&self) -> &[pipe::Event] { &self.raised_events }
    }

    impl endpoint::EndpointRegistrar for TestPipeContext {
        fn register(&mut self, _: &mio::Evented, interest: mio::Ready, opt: mio::PollOpt) {
            self.registrations.push((interest, opt));
        }
        fn reregister(&mut self, _: &mio::Evented, interest: mio::Ready, opt: mio::PollOpt) {
            self.reregistrations.push((interest, opt));
        }
        fn deregister(&mut self, _: &mio::Evented) {
            self.deregistrations += 1;
        }
    }

    impl fmt::Debug for TestPipeContext {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "TestPipeContext")
        }
    }

    impl pipe::Context for TestPipeContext {
        fn raise(&mut self, evt: pipe::Event) {
            self.raised_events.push(evt);
        }
    }
}