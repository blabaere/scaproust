// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use scaproust::*;

pub use super::urls;
pub use super::{make_session, make_timeout, sleep_some};

describe! chooses_the_correct_endpoint {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
        let url = urls::tcp::get();
        let timeout = make_timeout();
    }

    it "when activations are intertwined with recv" {
        let mut push1 = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut push2 = session.create_socket::<Push>().expect("Failed to create socket !");
        let mut push3 = session.create_socket::<Push>().expect("Failed to create socket !");

        pull.bind(&url).unwrap();
        push1.connect(&url).unwrap();
        push2.connect(&url).unwrap();
        push3.connect(&url).unwrap();

        sleep_some();

        push1.set_send_timeout(timeout).unwrap();
        push2.set_send_timeout(timeout).unwrap();
        push3.set_send_timeout(timeout).unwrap();
        pull.set_recv_timeout(timeout).unwrap();

        // Make sure the socket will try to read from the correct pipe
        push2.send(vec![65, 66, 67]).unwrap();
        let received1 = pull.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received1);

        // Make sure the socket will try to read from the same correct pipe
        push2.send(vec![67, 66, 65]).unwrap();
        let received2 = pull.recv().unwrap();
        assert_eq!(vec![67, 66, 65], received2);

        // Make sure the socket will try to read from the new correct pipe
        push1.send(vec![66, 67, 65]).unwrap();
        let received3 = pull.recv().unwrap();
        assert_eq!(vec![66, 67, 65], received3);

        // Make sure the socket will try to read from the newest correct pipe
        push3.send(vec![66, 67, 68]).unwrap();
        let received3 = pull.recv().unwrap();
        assert_eq!(vec![66, 67, 68], received3);
    }

    it "when activations are batched before several recv" {
        let mut push = session.create_socket::<Push>().expect("Failed to create socket !");

        pull.bind(&url).unwrap();
        push.connect(&url).unwrap();

        push.set_send_timeout(timeout).unwrap();
        pull.set_recv_timeout(timeout).unwrap();

        sleep_some();

        push.send(vec![65, 66, 67]).expect("First push failed");
        push.send(vec![67, 66, 65]).expect("Second push failed");

        let received1 = pull.recv().expect("First pull failed");
        assert_eq!(vec![65, 66, 67], received1);

        let received2 = pull.recv().expect("Second pull failed");
        assert_eq!(vec![67, 66, 65], received2);
    }

}
