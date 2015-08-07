extern crate scaproust;

use std::io;

use scaproust::*;

#[test]
fn test_pipeline_connected_to_bound() {
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
fn test_pipeline_bound_to_connected() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	push.bind("tcp://127.0.0.1:5455").unwrap();
	pull.connect("tcp://127.0.0.1:5455").unwrap();

	let sent = vec![65, 66, 67];
	push.send(sent).unwrap();
	let received = pull.recv().unwrap();

	assert_eq!(vec![65, 66, 67], received)
}

#[test]
fn test_send_while_not_connected() {
	let session = Session::new().unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	push.connect("tcp://127.0.0.1:5456").unwrap();

	let err = push.send(vec![65, 66, 67]).unwrap_err();

	assert_eq!(io::ErrorKind::NotConnected, err.kind());
}

#[test]
fn test_send_timeout() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	pull.bind("tcp://127.0.0.1:5457").unwrap();
	push.connect("tcp://127.0.0.1:5457").unwrap();

	let err = push.send(vec![0; 5 * 1024 * 1024]).unwrap_err();

	assert_eq!(io::ErrorKind::TimedOut, err.kind());
}

#[test]
fn test_recv_while_not_connected() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();

	pull.bind("tcp://127.0.0.1:5458").unwrap();

	let err = pull.recv().unwrap_err();

	assert_eq!(io::ErrorKind::NotConnected, err.kind());
}

#[test]
fn test_recv_timeout() {
	let session = Session::new().unwrap();
	let mut pull = session.create_socket(SocketType::Pull).unwrap();
	let mut push = session.create_socket(SocketType::Push).unwrap();

	pull.bind("tcp://127.0.0.1:5459").unwrap();
	push.connect("tcp://127.0.0.1:5459").unwrap();

	let err = pull.recv().unwrap_err();

	assert_eq!(io::ErrorKind::TimedOut, err.kind());
}

#[test]
fn test_pair_connected_to_bound() {
	let session = Session::new().unwrap();
	let mut bound = session.create_socket(SocketType::Pair).unwrap();
	let mut connected = session.create_socket(SocketType::Pair).unwrap();

	bound.bind("tcp://127.0.0.1:5460").unwrap();
	connected.connect("tcp://127.0.0.1:5460").unwrap();

	let sent = vec![65, 66, 67];
	connected.send(sent).unwrap();
	let received = bound.recv().unwrap();

	assert_eq!(vec![65, 66, 67], received)
}

#[test]
fn test_pair_bound_to_connected() {
	let session = Session::new().unwrap();
	let mut bound = session.create_socket(SocketType::Pair).unwrap();
	let mut connected = session.create_socket(SocketType::Pair).unwrap();

	bound.bind("tcp://127.0.0.1:5461").unwrap();
	connected.connect("tcp://127.0.0.1:5461").unwrap();

	let sent = vec![65, 66, 67];
	bound.send(sent).unwrap();
	let received = connected.recv().unwrap();

	assert_eq!(vec![65, 66, 67], received)
}
