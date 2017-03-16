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

fn before_each() -> (Session, String) {
    let _ = ::env_logger::init();
    let session = make_session();
    let url = urls::tcp::get();

    (session, url)
}

#[test]
fn send_can_complete_when_initiated_before_any_connection() {
    let (mut session, url) = before_each();
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
    drop(session);
}

#[test]
fn send_should_return_an_error_when_timed_out() {
    let (mut session, url) = before_each();
    let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
    let timeout = Some(Duration::from_millis(50));

    push.bind(&url).unwrap();
    push.set_send_timeout(timeout).unwrap();

    let err = push.send(vec![65, 66, 67]).unwrap_err();

    assert_eq!(io::ErrorKind::TimedOut, err.kind());
    drop(session);
}

#[test]
fn recv_can_complete_when_initiated_before_any_connection() {
    let (mut session, url) = before_each();
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
    drop(session);
}

#[test]
fn recv_should_return_an_error_when_timed_out() {
    let (mut session, url) = before_each();
    let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
    let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
    let timeout = Some(Duration::from_millis(50));

    pull.set_recv_timeout(timeout).unwrap();
    pull.bind(&url).unwrap();
    push.connect(&url).unwrap();
    sleep_some();

    let err = pull.recv().unwrap_err();

    assert_eq!(io::ErrorKind::TimedOut, err.kind());
    drop(session);
}

#[test]
fn try_send_return_would_block_when_no_peer_is_connected() {
    let (mut session, _) = before_each();
    let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
    let err = push.try_send(vec![65, 66, 67]).unwrap_err();

    assert_eq!(io::ErrorKind::WouldBlock, err.kind());
    drop(session);
}

#[test]
fn try_send_return_would_block_when_peer_buffer_is_full() {
    let (mut session, _) = before_each();
    let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
    let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
    let url = urls::tcp::get();

    push.bind(&url).unwrap();
    pull.connect(&url).unwrap();
    sleep_some();

    let mut sent = false;
    loop {
        match push.try_send(vec![6; 512]) {
            Ok(()) => {
                sent = true;
                continue;
            },
            Err(err) => {
                assert!(sent);
                assert_eq!(io::ErrorKind::WouldBlock, err.kind());
                break;
            }
        }
    }
    drop(session);
}

#[test]
fn try_recv_return_would_block_when_no_peer_is_connected() {
    let (mut session, _) = before_each();
    let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
    let err = pull.try_recv().unwrap_err();

    assert_eq!(io::ErrorKind::WouldBlock, err.kind());
    drop(session);
}

#[test]
fn try_recv_return_would_block_when_buffer_is_empty() {
    let (mut session, _) = before_each();
    let mut push = session.create_socket::<Push>().expect("Failed to create socket !");
    let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
    let url = urls::tcp::get();

    push.bind(&url).unwrap();
    pull.connect(&url).unwrap();
    sleep_some();

    let err = pull.try_recv().unwrap_err();
    assert_eq!(io::ErrorKind::WouldBlock, err.kind());
    drop(session);
}
