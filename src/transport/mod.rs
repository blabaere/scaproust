// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod async;
pub mod tcp;
pub mod endpoint;
pub mod pipe;
pub mod acceptor;

use std::io::Result;

pub const DEFAULT_RECV_MAX_SIZE: u64 = 1024 * 1024;

pub trait Transport {
    fn connect(&self, url: &str, pids: (u16, u16)) -> Result<Box<pipe::Pipe>>;
    fn bind(&self, url: &str, pids: (u16, u16)) -> Result<Box<acceptor::Acceptor>>;
}

#[cfg(test)]
mod tests {
    use std::io;

    use mio;

    use transport::*;
    use io_error::*;

    pub struct TestPipeContext {
        registration_ok: bool,
        reregistration_ok: bool,
        deregistration_ok: bool,
        registrations: Vec<(mio::EventSet, mio::PollOpt)>,
        reregistrations: Vec<(mio::EventSet, mio::PollOpt)>,
        deregistrations: usize,
        raised_events: Vec<pipe::Event>
    }

    impl TestPipeContext {
        pub fn new() -> TestPipeContext {
            TestPipeContext {
                registration_ok: true,
                reregistration_ok: true,
                deregistration_ok: true,
                registrations: Vec::new(),
                reregistrations: Vec::new(),
                deregistrations: 0,
                raised_events: Vec::new()
            }
        }
        pub fn set_registration_ok(&mut self, registration_ok: bool) { self.registration_ok = registration_ok; }
        pub fn set_reregistration_ok(&mut self, reregistration_ok: bool) { self.reregistration_ok = reregistration_ok; }
        pub fn set_deregistration_ok(&mut self, deregistration_ok: bool) { self.deregistration_ok = deregistration_ok; }
        pub fn get_registrations(&self) -> &[(mio::EventSet, mio::PollOpt)] { &self.registrations }
        pub fn get_reregistrations(&self) -> &[(mio::EventSet, mio::PollOpt)] { &self.reregistrations }
        pub fn get_deregistrations(&self) -> usize { self.deregistrations }
        pub fn get_raised_events(&self) -> &[pipe::Event] { &self.raised_events }
    }

    impl endpoint::EndpointRegistrar for TestPipeContext {
        fn register(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
            self.registrations.push((interest, opt));
            if self.registration_ok { Ok(()) } else { Err(other_io_error("test")) }
        }
        fn reregister(&mut self, io: &mio::Evented/*, tok: mio::Token*/, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
            self.reregistrations.push((interest, opt));
            if self.reregistration_ok { Ok(()) } else { Err(other_io_error("test")) }
        }
        fn deregister(&mut self, io: &mio::Evented) -> io::Result<()> {
            self.deregistrations += 1;
            if self.deregistration_ok { Ok(()) } else { Err(other_io_error("test")) }
        }
    }

    impl pipe::Context for TestPipeContext {
        fn raise(&mut self, evt: pipe::Event) {
            self.raised_events.push(evt);
        }
    }
}