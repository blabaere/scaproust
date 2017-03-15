// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc;
use std::io;

use core::{BuildIdHasher, SocketId, DeviceId, ProbeId, PollReq, socket, device, probe};
use sequence::Sequence;

pub enum Request {
    CreateSocket(socket::ProtocolCtor),
    CreateDevice(SocketId, SocketId),
    CreateProbe(Vec<PollReq>),
    Shutdown
}

pub enum Reply {
    Err(io::Error),
    SocketCreated(SocketId, mpsc::Receiver<socket::Reply>),
    DeviceCreated(DeviceId, mpsc::Receiver<device::Reply>),
    ProbeCreated(ProbeId, mpsc::Receiver<probe::Reply>),
    Shutdown
}

pub struct Session {
    reply_sender: mpsc::Sender<Reply>,
    sockets: SocketCollection,
    devices: DeviceCollection,
    probes: ProbeCollection
}

struct SocketCollection {
    ids: Sequence,
    sockets: HashMap<SocketId, socket::Socket, BuildIdHasher>
}

struct DeviceCollection {
    ids: Sequence,
    mapping: HashMap<SocketId, DeviceId, BuildIdHasher>,
    devices: HashMap<DeviceId, device::Device, BuildIdHasher>
}

struct ProbeCollection {
    ids: Sequence,
    mapping: HashMap<SocketId, ProbeId, BuildIdHasher>,
    probes: HashMap<ProbeId, probe::Probe, BuildIdHasher>
}

impl Session {
    pub fn new(seq: Sequence, reply_tx: mpsc::Sender<Reply>) -> Session {
        Session {
            reply_sender: reply_tx,
            sockets: SocketCollection::new(seq.clone()),
            devices: DeviceCollection::new(seq.clone()),
            probes: ProbeCollection::new(seq.clone())
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
        let protocol = protocol_ctor(tx.clone());
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

/*****************************************************************************/
/*                                                                           */
/* Probes                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn add_probe(&mut self, poll_opts: Vec<PollReq>) {
        let (tx, rx) = mpsc::channel();
        let id = self.probes.add(tx, poll_opts);

        self.send_reply(Reply::ProbeCreated(id, rx));
    }

    pub fn get_probe_mut(&mut self, id: ProbeId) -> Option<&mut probe::Probe> {
        self.probes.get_probe_mut(id)
    }

    pub fn find_probe_mut(&mut self, id: SocketId) -> Option<(&ProbeId, &mut probe::Probe)> {
        self.probes.find_probe_mut(id)
    }

    pub fn remove_probe(&mut self, id: ProbeId) {
        self.probes.remove(id);
    }
}

/*****************************************************************************/
/*                                                                           */
/* Socket collection                                                         */
/*                                                                           */
/*****************************************************************************/

impl SocketCollection {
    fn new(seq: Sequence) -> SocketCollection {
        SocketCollection {
            ids: seq,
            sockets: HashMap::default()
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

/*****************************************************************************/
/*                                                                           */
/* Device collection                                                         */
/*                                                                           */
/*****************************************************************************/

impl DeviceCollection {
    fn new(seq: Sequence) -> DeviceCollection {
        DeviceCollection {
            ids: seq,
            mapping: HashMap::default(),
            devices: HashMap::default()
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
        if let Some(device) = self.devices.remove(&id) {
            self.mapping.remove(device.get_left_id());
            self.mapping.remove(device.get_right_id());
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Probe collection                                                          */
/*                                                                           */
/*****************************************************************************/

impl ProbeCollection {
    fn new(seq: Sequence) -> ProbeCollection {
        ProbeCollection {
            ids: seq,
            mapping: HashMap::default(),
            probes: HashMap::default()
        }
    }

    fn add(&mut self, reply_tx: mpsc::Sender<probe::Reply>, poll_opts: Vec<PollReq>) -> ProbeId {
        let id = ProbeId::from(self.ids.next());
        
        for poll_opt in &poll_opts {
            self.mapping.insert(poll_opt.sid, id);
        }

        let probe = probe::Probe::new(reply_tx, poll_opts);

        self.probes.insert(id, probe);

        id
    }

    fn get_probe_mut(&mut self, id: ProbeId) -> Option<&mut probe::Probe> {
        self.probes.get_mut(&id)
    }

    fn find_probe_mut(&mut self, sid: SocketId) -> Option<(&ProbeId, &mut probe::Probe)> {
        if let Some(pid) = self.mapping.get(&sid) {
            self.probes.get_mut(pid).map(|probe| (pid, probe))
        } else {
            None
        }
    }

    fn remove(&mut self, id: ProbeId) {
        if let Some(probe) = self.probes.remove(&id) {
            let ids = probe.get_socket_ids();

            for id in &ids {
                self.mapping.remove(id);
            }
        }
    }
}

