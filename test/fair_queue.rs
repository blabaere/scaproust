// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


pub use scaproust::*;

pub use super::urls;
pub use super::sleep_some;

#[test]
fn check_readable_pipe_is_used_for_recv() {
    let mut session = SessionBuilder::build().expect("Failed to create session !");
    let mut pull = session.create_socket::<Pull>().expect("Failed to create socket !");
    let mut push1 = session.create_socket::<Push>().expect("Failed to create socket !");
    let mut push2 = session.create_socket::<Push>().expect("Failed to create socket !");
    let mut push3 = session.create_socket::<Push>().expect("Failed to create socket !");
    let timeout = Some(Duration::from_millis(500));

    pull.bind("tcp://127.0.0.1:5473").unwrap();
    push1.connect("tcp://127.0.0.1:5473").unwrap();
    push2.connect("tcp://127.0.0.1:5473").unwrap();
    push3.connect("tcp://127.0.0.1:5473").unwrap();

    thread::sleep(Duration::from_millis(250));
    //sleep_enough_for_connections_to_establish();

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
