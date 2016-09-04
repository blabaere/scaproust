// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

pub use super::{urls, make_session, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let url = urls::tcp::get();
        let timeout = make_timeout();

        push.set_send_timeout(timeout).expect("Failed to set send timeout !");
        pull.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "send a message through local endpoint" {
        push.bind(&url).unwrap();
        pull.connect(&url).unwrap();

        let sent = vec![65, 66, 67];
        push.send(sent).unwrap();
        let received = pull.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received)
    }

    it "send a message through remote endpoint" {
        pull.bind(&url).unwrap();
        push.connect(&url).unwrap();

        let sent = vec![65, 66, 67];
        push.send(sent).unwrap();
        let received = pull.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received)
    }
}