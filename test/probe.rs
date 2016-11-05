// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


pub use std::time::Duration;
pub use std::thread;
pub use std::io;
pub use std::sync::{Arc, Barrier};

pub use scaproust::*;

pub use super::urls;
pub use super::{make_session, make_hard_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let timeout = make_hard_timeout();
    }

    it "talk with the backend" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let requests = vec![push.create_poll_req(false, true), push.create_poll_req(true, false)];
        let mut probe = session.create_probe(requests).expect("Failed to create probe !");

        let poll_result = probe.poll(timeout);

        poll_result.expect("poll should have succeed");
    }

}