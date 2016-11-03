// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

pub use super::urls;
pub use super::{make_session, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let mut busl = session.create_socket::<Bus>().expect("Failed to create socket !");
        let mut bus2 = session.create_socket::<Bus>().expect("Failed to create socket !");
        let mut bus3 = session.create_socket::<Bus>().expect("Failed to create socket !");
        let timeout = make_timeout();

        busl.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        bus2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        bus3.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "broadcast a message through local endpoint" {
        let url = urls::tcp::get();

        busl.bind(&url).unwrap();
        bus2.connect(&url).unwrap();
        bus3.connect(&url).unwrap();

        sleep_some();

        let sent = vec![65, 66, 67];
        busl.send(sent).unwrap();
        let received2 = bus2.recv().unwrap();
        let received3 = bus3.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received2);
        assert_eq!(vec![65, 66, 67], received3);
    }

    it "broadcast a message through remote endpoint" {
        let url2 = urls::tcp::get();
        let url3 = urls::tcp::get();

        bus2.bind(&url2).unwrap();
        bus3.bind(&url3).unwrap();

        busl.connect(&url2).unwrap();
        busl.connect(&url3).unwrap();

        sleep_some();

        let sent = vec![65, 66, 67];
        busl.send(sent).unwrap();
        let received2 = bus2.recv().unwrap();
        let received3 = bus3.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received2);
        assert_eq!(vec![65, 66, 67], received3);
    }

    it "avoid to broadcast back to originator" {
        let url1 = urls::tcp::get();
        let url2 = urls::tcp::get();
        let url3 = urls::tcp::get();

        busl.bind(&url1).unwrap();
        bus2.bind(&url2).unwrap();
        bus3.bind(&url3).unwrap();

        bus2.connect(&url1).unwrap(); // 1 <-> 2
        bus2.connect(&url3).unwrap(); // 2 <-> 3
        bus3.connect(&url1).unwrap(); // 1 <-> 3

        sleep_some();

        let sent = vec![65, 66, 67];
        busl.send(sent).expect("busl should have sent a message");
        let _ = bus2.recv_msg().expect("bus2 should have received a message");
        let received3 = bus3.recv_msg().expect("bus3 should have received a message");

        sleep_some();

        bus3.send_msg(received3).expect("bus3 should have sent a message");

        let not_received1 = busl.recv().unwrap_err();
        let received2 = bus2.recv().expect("bus2 should have received a message");

        assert_eq!(io::ErrorKind::TimedOut, not_received1.kind());
        assert_eq!(vec![65, 66, 67], received2);
    }
}