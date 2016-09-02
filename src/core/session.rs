// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc;
use std::io;

use core::{SocketId, DeviceId, socket, device};
use sequence::Sequence;

pub enum Request {
    CreateSocket(socket::ProtocolCtor),
    CreateDevice(SocketId, SocketId),
    Shutdown
}

pub enum Reply {
    Err(io::Error),
    SocketCreated(SocketId, mpsc::Receiver<socket::Reply>),
    DeviceCreated(DeviceId, mpsc::Receiver<device::Reply>),
    Shutdown
}

pub struct Session {
    reply_sender: mpsc::Sender<Reply>,
    sockets: SocketCollection,
    devices: DeviceCollection
}

struct SocketCollection {
    ids: Sequence,
    sockets: HashMap<SocketId, socket::Socket>
}

struct DeviceCollection {
    ids: Sequence,
    mapping: HashMap<SocketId, DeviceId>,
    devices: HashMap<DeviceId, device::Device>
}

impl Session {
    pub fn new(seq: Sequence, reply_tx: mpsc::Sender<Reply>) -> Session {
        Session {
            reply_sender: reply_tx,
            sockets: SocketCollection::new(seq.clone()),
            devices: DeviceCollection::new(seq.clone())
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

/*****************************************************************************/
/*                                                                           */
/* Sockets                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn add_socket(&mut self, protocol_ctor: socket::ProtocolCtor) {
        let (tx, rx) = mpsc::channel();
        let protocol_ctor_args = (tx.clone(),);
        let protocol = protocol_ctor.call_box(protocol_ctor_args);
        let id = self.sockets.add(tx, protocol);

        self.send_reply(Reply::SocketCreated(id, rx));
    }

    pub fn get_socket_mut(&mut self, id: SocketId) -> Option<&mut socket::Socket> {
        self.sockets.get_socket_mut(id)
    }

    pub fn remove_socket(&mut self, sid: SocketId) {
        self.sockets.remove(sid);
    }

/*****************************************************************************/
/*                                                                           */
/* Devices                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn add_device(&mut self, left: SocketId, right: SocketId) {
        let (tx, rx) = mpsc::channel();
        let id = self.devices.add(tx, left, right);

        self.send_reply(Reply::DeviceCreated(id, rx));
    }

    pub fn get_device_mut(&mut self, id: DeviceId) -> Option<&mut device::Device> {
        self.devices.get_device_mut(id)
    }

    pub fn find_device_mut(&mut self, id: SocketId) -> Option<&mut device::Device> {
        self.devices.find_device_mut(id)
    }

    pub fn remove_device(&mut self, did: DeviceId) {
        self.devices.remove(did);
    }
}

impl SocketCollection {
    fn new(seq: Sequence) -> SocketCollection {
        SocketCollection {
            ids: seq,
            sockets: HashMap::new()
        }
    }

    fn add(&mut self, reply_tx: mpsc::Sender<socket::Reply>, proto: Box<socket::Protocol>) -> SocketId {
        let id = SocketId::from(self.ids.next());
        let socket = socket::Socket::new(id, reply_tx, proto);

        self.sockets.insert(id, socket);

        id
    }

    fn get_socket_mut(&mut self, id: SocketId) -> Option<&mut socket::Socket> {
        self.sockets.get_mut(&id)
    }

    fn remove(&mut self, id: SocketId) {
        self.sockets.remove(&id);
    }
}

impl DeviceCollection {
    fn new(seq: Sequence) -> DeviceCollection {
        DeviceCollection {
            ids: seq,
            mapping: HashMap::new(),
            devices: HashMap::new()
        }
    }

    fn add(&mut self, reply_tx: mpsc::Sender<device::Reply>, left: SocketId, right: SocketId) -> DeviceId {
        let id = DeviceId::from(self.ids.next());
        let device = device::Device::new(reply_tx, left, right);

        self.devices.insert(id, device);
        self.mapping.insert(left, id);
        self.mapping.insert(right, id);

        id
    }

    fn get_device_mut(&mut self, id: DeviceId) -> Option<&mut device::Device> {
        self.devices.get_mut(&id)
    }

    fn find_device_mut(&mut self, sid: SocketId) -> Option<&mut device::Device> {
        if let Some(did) = self.mapping.get(&sid) {
            self.devices.get_mut(did)
        } else {
            None
        }
    }

    fn remove(&mut self, id: DeviceId) {
        self.devices.remove(&id);
        // TODO cleaup mapping
    }
}
