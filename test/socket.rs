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

describe! send {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let url = urls::tcp::get();
    }

    it "can complete when initiated before any connection" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let pull_url = url.clone();

        let pull_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            pull.set_recv_timeout(make_timeout()).unwrap();
            pull.connect(&pull_url).unwrap();
            let received = pull.recv().unwrap();
            assert_eq!(vec![65, 66, 67], received)
        });

        push.set_send_timeout(make_timeout()).unwrap();
        push.bind(&url).unwrap();
        push.send(vec![65, 66, 67]).unwrap();

        pull_thread.join().unwrap();
    }

    it "should return an error when timed out" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let timeout = Some(Duration::from_millis(50));

        push.bind(&url).unwrap();
        push.set_send_timeout(timeout).unwrap();

        let err = push.send(vec![65, 66, 67]).unwrap_err();

        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

}

describe! recv {

    before_each {
        let mut session = make_session();
        let url = urls::tcp::get();
    }

    it "can complete when initiated before any connection" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let push_url = url.clone();

        pull.set_recv_timeout(make_timeout()).unwrap();
        pull.bind(&url).unwrap();

        let push_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));

            push.set_send_timeout(make_timeout()).unwrap();
            push.connect(&push_url).unwrap();
            push.send(vec![65, 66, 67]).unwrap();

            thread::sleep(Duration::from_millis(50));
        });

        let received = pull.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received);

        push_thread.join().unwrap();
    }

    it "should return an error when timed out" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let timeout = Some(Duration::from_millis(50));

        pull.set_recv_timeout(timeout).unwrap();
        pull.bind(&url).unwrap();
        push.connect(&url).unwrap();
        sleep_some();

        let err = pull.recv().unwrap_err();

        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

}

