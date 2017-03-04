// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
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
pub use super::{make_session, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let timeout = make_timeout();
    }

    it "relay messages" {
        let mut server = session.create_socket::<Bus>().expect("Failed to create socket !");
        let mut client1 = session.create_socket::<Bus>().expect("Failed to create socket !");
        let mut client2 = session.create_socket::<Bus>().expect("Failed to create socket !");
        let url = urls::tcp::get();

        server.bind(&url).unwrap();
        client1.connect(&url).unwrap();
        client2.connect(&url).unwrap();

        client1.set_send_timeout(timeout).unwrap();
        client2.set_send_timeout(timeout).unwrap();
        client1.set_recv_timeout(timeout).unwrap();
        client2.set_recv_timeout(timeout).unwrap();

        sleep_some();

        let device = session.create_relay_device(server).unwrap();
        let device_thread = thread::spawn(move || device.run());

        sleep_some();

        client1.send(vec![65, 66, 67]).unwrap();
        let received = client2.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received);

        let err = client1.recv().unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());

        drop(session); // this is required to stop the device
        device_thread.join().unwrap().unwrap_err();
    }

    it "forward messages" {

        let mut d_push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut d_pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");

        let d_push_url = urls::tcp::get();
        let d_pull_url = urls::tcp::get();

        d_push.bind(&d_push_url).unwrap();
        d_pull.bind(&d_pull_url).unwrap();

        push.set_send_timeout(timeout).unwrap();
        pull.set_recv_timeout(timeout).unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let d_barrier = barrier.clone();
        let device = session.create_bridge_device(d_pull, d_push).unwrap();
        let device_thread = thread::spawn(move || {
            d_barrier.wait();
            let res = device.run();
            res
        });

        barrier.wait();
        sleep_some();

        push.connect(&d_pull_url).unwrap();
        pull.connect(&d_push_url).unwrap();
        sleep_some();

        push.send(vec![65, 66, 67]).expect("Push should have sent a message");
        let received = pull.recv().expect("Pull should have received a message");
        assert_eq!(vec![65, 66, 67], received);

        let err = pull.recv().unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());

        drop(session);
        device_thread.join().unwrap().unwrap_err();
    }

    it "forward messages back and forth" {

        let mut d_req = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut d_rep = session.create_socket::<Rep>().expect("Failed to create socket !");
        let mut req = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut rep = session.create_socket::<Rep>().expect("Failed to create socket !");

        let d_req_url = urls::tcp::get();
        let d_rep_url = urls::tcp::get();

        d_req.bind(&d_req_url).unwrap();
        d_rep.bind(&d_rep_url).unwrap();

        req.set_send_timeout(timeout).unwrap();
        req.set_recv_timeout(timeout).unwrap();
        rep.set_send_timeout(timeout).unwrap();
        rep.set_recv_timeout(timeout).unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let d_barrier = barrier.clone();
        let device = session.create_bridge_device(d_rep, d_req).unwrap();
        let device_thread = thread::spawn(move || {
            d_barrier.wait();
            let res = device.run();
            res
        });

        barrier.wait();
        sleep_some();

        req.connect(&d_rep_url).unwrap();
        rep.connect(&d_req_url).unwrap();
        sleep_some();

        let sent_request = vec![65, 66, 67];
        req.send(sent_request).expect("Req should have sent a request");
        let received_request = rep.recv().expect("Rep should have received a request");
        assert_eq!(vec![65, 66, 67], received_request);

        let sent_reply = vec![66, 65, 67];
        rep.send(sent_reply).expect("Rep should have sent a reply");
        let received_reply = req.recv().expect("Req should have received a reply");
        assert_eq!(vec![66, 65, 67], received_reply);

        drop(session);
        device_thread.join().unwrap().unwrap_err();
    }
}