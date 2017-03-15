// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
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

fn before_each() -> (Session, Socket, Socket, Socket) {
    let _ = ::env_logger::init();
    let mut session = make_session();
    let mut bus1 = session.create_socket::<Bus>().expect("Failed to create socket !");
    let mut bus2 = session.create_socket::<Bus>().expect("Failed to create socket !");
    let mut bus3 = session.create_socket::<Bus>().expect("Failed to create socket !");
    let timeout = make_timeout();

    bus1.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    bus2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    bus3.set_recv_timeout(timeout).expect("Failed to set recv timeout !");

    (session, bus1, bus2, bus3)
}

#[test]
fn broadcast_a_message_through_local_endpoint() {
    let (session, mut bus1, mut bus2, mut bus3) = before_each();
    let url = urls::tcp::get();

    bus1.bind(&url).unwrap();
    bus2.connect(&url).unwrap();
    bus3.connect(&url).unwrap();

    sleep_some();

    let sent = vec![65, 66, 67];
    bus1.send(sent).unwrap();
    let received2 = bus2.recv().unwrap();
    let received3 = bus3.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received2);
    assert_eq!(vec![65, 66, 67], received3);
    drop(session);
}

#[test]
fn broadcast_a_message_through_remote_endpoint() {
    let (session, mut bus1, mut bus2, mut bus3) = before_each();
    let url2 = urls::tcp::get();
    let url3 = urls::tcp::get();

    bus2.bind(&url2).unwrap();
    bus3.bind(&url3).unwrap();

    bus1.connect(&url2).unwrap();
    bus1.connect(&url3).unwrap();

    sleep_some();

    let sent = vec![65, 66, 67];
    bus1.send(sent).unwrap();
    let received2 = bus2.recv().unwrap();
    let received3 = bus3.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received2);
    assert_eq!(vec![65, 66, 67], received3);
    drop(session);
}

#[test]
fn avoid_to_broadcast_back_to_originator() {
    let (session, mut bus1, mut bus2, mut bus3) = before_each();
    let url1 = urls::tcp::get();
    let url2 = urls::tcp::get();
    let url3 = urls::tcp::get();

    bus1.bind(&url1).unwrap();
    bus2.bind(&url2).unwrap();
    bus3.bind(&url3).unwrap();

    bus2.connect(&url1).unwrap(); // 1 <-> 2
    bus2.connect(&url3).unwrap(); // 2 <-> 3
    bus3.connect(&url1).unwrap(); // 1 <-> 3

    sleep_some();

    let sent = vec![65, 66, 67];
    bus1.send(sent).expect("bus1 should have sent a message");
    let _ = bus2.recv_msg().expect("bus2 should have received a message");
    let received3 = bus3.recv_msg().expect("bus3 should have received a message");

    sleep_some();

    bus3.send_msg(received3).expect("bus3 should have sent a message");

    let not_received1 = bus1.recv().unwrap_err();
    let received2 = bus2.recv().expect("bus2 should have received a message");

    assert_eq!(io::ErrorKind::TimedOut, not_received1.kind());
    assert_eq!(vec![65, 66, 67], received2);
    drop(session);
}
