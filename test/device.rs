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
pub use super::{sleep_some, make_timeout};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = SessionBuilder::build().expect("Failed to create session !");
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
}