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
pub use super::{make_session, make_hard_timeout, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let timeout = make_hard_timeout();
    }

    it "report socket readiness" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let requests = vec![
            push.create_poll_req(false, true), 
            pull.create_poll_req(true, false)];
        let mut probe = session.create_probe(requests).expect("Failed to create probe !");
        let url = urls::tcp::get();

        push.set_send_timeout(make_timeout()).expect("Failed to set send timeout !");
        pull.set_recv_timeout(make_timeout()).expect("Failed to set recv timeout !");

        push.bind(&url).unwrap();
        pull.connect(&url).unwrap();

        let poll_result = probe.poll(timeout).expect("Before send, poll should have succeed");
        assert_eq!(2, poll_result.len());
        assert!(!poll_result[0].recv, "Before send, Push should not be recv ready");
        assert!(poll_result[0].send,  "Before send, Push should be send ready");
        assert!(!poll_result[1].recv, "Before send, Pull should not be recv ready");
        assert!(!poll_result[1].send, "Before send, Pull should not be send ready");

        push.send_msg(Message::new()).expect("Failed to send a message !");

        let poll_result = probe.poll(timeout).expect("After send, poll should have succeed");
        assert_eq!(2, poll_result.len());
        assert!(!poll_result[0].recv, "After send, Push should not be recv ready");
        assert!(poll_result[0].send,  "After send, Push should be send ready");
        assert!(poll_result[1].recv,  "After send, Pull should be recv ready");
        assert!(!poll_result[1].send, "After send, Pull should not be send ready");

        pull.recv_msg().expect("Failed to recv a message !");

        let poll_result = probe.poll(timeout).expect("After recv, poll should have succeed");
        assert_eq!(2, poll_result.len());
        assert!(!poll_result[0].recv, "After recv, Push should not be recv ready");
        assert!(poll_result[0].send,  "After recv, Push should be send ready");
        assert!(!poll_result[1].recv, "After recv, Pull should not be recv ready");
        assert!(!poll_result[1].send, "After recv, Pull should not be send ready");
    }

}