// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
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
        let mut publ = session.create_socket::<Pub>().expect("Failed to create socket !");
        let mut sub1 = session.create_socket::<Sub>().expect("Failed to create socket !");
        let mut sub2 = session.create_socket::<Sub>().expect("Failed to create socket !");
        let mut sub3 = session.create_socket::<Sub>().expect("Failed to create socket !");
        let timeout = make_timeout();

        sub1.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        sub2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        sub3.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "broadcast a message through local endpoint" {
        let url = urls::tcp::get();

        publ.bind(&url).unwrap();
        sub1.connect(&url).unwrap();
        sub2.connect(&url).unwrap();
        sub3.connect(&url).unwrap();

        sub1.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();
        sub2.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();
        sub3.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();

        sleep_some();

        let sent = vec![65, 66, 67];
        publ.send(sent).unwrap();
        let received1 = sub1.recv().unwrap();
        let received2 = sub2.recv().unwrap();
        let received3 = sub3.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received1);
        assert_eq!(vec![65, 66, 67], received2);
        assert_eq!(vec![65, 66, 67], received3);
    }

    it "broadcast a message through remote endpoint" {
        let url1 = urls::tcp::get();
        let url2 = urls::tcp::get();
        let url3 = urls::tcp::get();

        sub1.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();
        sub2.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();
        sub3.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();

        sub1.bind(&url1).unwrap();
        sub2.bind(&url2).unwrap();
        sub3.bind(&url3).unwrap();
        publ.connect(&url1).unwrap();
        publ.connect(&url2).unwrap();
        publ.connect(&url3).unwrap();

        sleep_some();

        let sent = vec![65, 66, 67];
        publ.send(sent).unwrap();
        let received1 = sub1.recv().unwrap();
        let received2 = sub2.recv().unwrap();
        let received3 = sub3.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received1);
        assert_eq!(vec![65, 66, 67], received2);
        assert_eq!(vec![65, 66, 67], received3);
    }

    it "ignores messages based on subscriptions" {
        let url = urls::tcp::get();

        publ.bind(&url).unwrap();
        sub1.connect(&url).unwrap();
        sub2.connect(&url).unwrap();
        sub3.connect(&url).unwrap();

        sub1.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();
        sub2.set_option(ConfigOption::Subscribe(String::from("B"))).unwrap();
        sub3.set_option(ConfigOption::Subscribe(String::from("A"))).unwrap();

        sleep_some();

        let sent = vec![65, 66, 67];
        publ.send(sent).unwrap();
        let received1 = sub1.recv().unwrap();
        let not_received2 = sub2.recv().unwrap_err();
        let received3 = sub3.recv().unwrap();

        assert_eq!(vec![65, 66, 67], received1);
        assert_eq!(io::ErrorKind::TimedOut, not_received2.kind());
        assert_eq!(vec![65, 66, 67], received3);
    }
}