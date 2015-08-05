extern crate scaproust;

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