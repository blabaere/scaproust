extern crate scaproust;

use std::io;
use std::error::*;

use scaproust::*;


#[test]
fn test_push_pull_one_to_one() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	pull.bind("tcp://127.0.0.1:5454").unwrap();
	push.connect("tcp://127.0.0.1:5454").unwrap();

	let sent = vec![65, 66, 67];
	push.send(sent).unwrap();
	let received = pull.recv().unwrap();

	assert_eq!(vec![65, 66, 67], received)
}

#[test]
fn test_send_while_not_connected() {
	let session = Session::new().unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	push.connect("tcp://127.0.0.1:5455").unwrap();

	let err = push.send(vec![65, 66, 67]).unwrap_err();

	assert_eq!(io::ErrorKind::NotConnected, err.kind());
}

#[test]
fn test_send_timeout() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	pull.bind("tcp://127.0.0.1:5456").unwrap();
	push.connect("tcp://127.0.0.1:5456").unwrap();

	let err = push.send(vec![0; 5 * 1024 * 1024]).unwrap_err();

	assert_eq!(io::ErrorKind::TimedOut, err.kind());
}

#[test]
fn test_recv_while_not_connected() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();

	pull.bind("tcp://127.0.0.1:5457").unwrap();

	let err = pull.recv().unwrap_err();

	assert_eq!(io::ErrorKind::NotConnected, err.kind());
}

#[test]
fn test_recv_timeout() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	pull.bind("tcp://127.0.0.1:5458").unwrap();
	push.connect("tcp://127.0.0.1:5458").unwrap();

	let err = pull.recv().unwrap_err();

	assert_eq!(io::ErrorKind::TimedOut, err.kind());
}