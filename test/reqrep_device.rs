// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;
pub use std::sync::{Arc, Barrier};

pub use scaproust::*;

pub use super::{urls, make_session, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let mut req_1 = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut req_2 = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut d_rep = session.create_socket::<Rep>().expect("Failed to create socket !");
        let mut d_req = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut rep_1 = session.create_socket::<Rep>().expect("Failed to create socket !");
        let mut rep_2 = session.create_socket::<Rep>().expect("Failed to create socket !");
        let l_url = urls::tcp::get();
        let r_url = urls::tcp::get();
        let timeout = make_timeout();

        req_1.set_send_timeout(timeout).expect("Failed to set send timeout !");
        req_2.set_send_timeout(timeout).expect("Failed to set send timeout !");
        rep_1.set_send_timeout(timeout).expect("Failed to set send timeout !");
        rep_2.set_send_timeout(timeout).expect("Failed to set send timeout !");
        req_1.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        req_2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        rep_1.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        rep_2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "create a many to many topology" {
        d_rep.bind(&l_url).unwrap();
        d_req.bind(&r_url).unwrap();
        req_1.connect(&l_url).unwrap();
        rep_1.connect(&r_url).unwrap();
        sleep_some();

        req_2.connect(&l_url).unwrap();
        rep_2.connect(&r_url).unwrap();

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

        req_1.send(vec![65, 66, 67]).expect("req_1 should have sent a request");
        let received = rep_1.recv().expect("rep_1 should have received a request");
        assert_eq!(vec![65, 66, 67], received);

        req_2.send(vec![65, 66, 68]).expect("req_2 should have sent a request");
        let received = rep_2.recv().expect("rep_2 should have received a request");
        assert_eq!(vec![65, 66, 68], received);

        rep_2.send(vec![66, 66, 66]).expect("rep_2 should have sent a reply");
        let received = req_2.recv().expect("req_2 should have received a reply");
        assert_eq!(vec![66, 66, 66], received);

        rep_1.send(vec![69, 69, 69]).expect("rep_1 should have sent a reply");
        let received = req_1.recv().expect("req_1 should have received a reply");
        assert_eq!(vec![69, 69, 69], received);

        drop(session); // this is required to stop the device
        device_thread.join().unwrap().unwrap_err();
    }
}