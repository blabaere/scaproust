// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::fmt;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io::{Error, Result};
use std::time::Duration;

use super::{BuildIdHasher, SocketId, PollReq, PollRes, Scheduled};

pub enum Request {
    Poll(Duration),
    Close
}

pub enum Reply {
    Err(Error),
    Poll(Vec<PollRes>),
    Closed
}

pub enum Schedulable {
    PollTimeout
}

pub trait Scheduler {
    fn schedule(&mut self, schedulable: Schedulable, delay: Duration) -> Result<Scheduled>;
    fn cancel(&mut self, scheduled: Scheduled);
}

pub trait Context : Scheduler + fmt::Debug {
    fn poll(&mut self, sid: SocketId);
}

pub struct Probe {
    reply_sender: Sender<Reply>,
    poll_opts: Vec<PollReq>,
    recv_votes: Vec<Option<bool>>,
    send_votes: Vec<Option<bool>>,
    sid_to_idx: HashMap<SocketId, usize, BuildIdHasher>,
    timeout: Option<Scheduled>
}

impl Probe {
    pub fn new(reply_tx: Sender<Reply>, poll_opts: Vec<PollReq>) -> Probe {
        let mut mappings = HashMap::default();
        let mut recv_votes = Vec::with_capacity(poll_opts.len());
        let mut send_votes = Vec::with_capacity(poll_opts.len());

        for (i, poll_opt) in poll_opts.iter().enumerate() {
            mappings.insert(poll_opt.sid, i);
            recv_votes.push(None);
            send_votes.push(None);
        }

        Probe {
            reply_sender: reply_tx,
            sid_to_idx: mappings,
            poll_opts: poll_opts,
            recv_votes: recv_votes,
            send_votes: send_votes,
            timeout: None
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn poll(&mut self, ctx: &mut Context, delay: Duration) {
        #[cfg(debug_assertions)] debug!("[{:?}] poll", ctx);
        let task = Schedulable::PollTimeout;

        match ctx.schedule(task, delay) {
            Ok(timeout) => self.start_poll(ctx, timeout),
            Err(e) => self.send_reply(Reply::Err(e))
        }
    }

    fn start_poll(&mut self, ctx: &mut Context, timeout: Scheduled) {
        self.timeout = Some(timeout);

        for po in &self.poll_opts {
            ctx.poll(po.sid);
        }

        self.check(ctx);
    }

    pub fn on_poll_timeout(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] on_poll_timeout", ctx);

        let poll_results = self.poll_opts.iter().enumerate().map(|(i, po)| { 
            PollRes {
                recv: is_socket_side_ready(po.recv, &self.recv_votes[i]),
                send: is_socket_side_ready(po.send, &self.send_votes[i])
            }
        }).collect();

        self.on_poll_succeed(ctx, poll_results);
    }

    pub fn on_socket_can_recv(&mut self, ctx: &mut Context, sid: SocketId, can_recv: bool) {
        #[cfg(debug_assertions)] debug!("[{:?}] on_socket_can_recv {:?} {}", ctx, sid, can_recv);
        if let Some(i) = self.sid_to_idx.get(&sid) {
            self.recv_votes[*i] = Some(can_recv);
        }

        self.check(ctx);
    }

    pub fn on_socket_can_send(&mut self, ctx: &mut Context, sid: SocketId, can_send: bool) {
        #[cfg(debug_assertions)] debug!("[{:?}] on_socket_can_send {:?} {}", ctx, sid, can_send);
        if let Some(i) = self.sid_to_idx.get(&sid) {
            self.send_votes[*i] = Some(can_send);
        }

        self.check(ctx);
    }

    fn check(&mut self, ctx: &mut Context) {
        if self.timeout.is_none() {
            return;
        }

        for (i, poll_opt) in self.poll_opts.iter().enumerate() {
            if !is_vote_over(poll_opt.recv, &self.recv_votes[i]) {
                return;
            }
            if !is_vote_over(poll_opt.send, &self.send_votes[i]) {
                return;
            }
        }

        #[cfg(debug_assertions)] debug!("[{:?}] check succeed", ctx);

        let poll_results = self.poll_opts.iter().map(|po| { 
            PollRes {
                recv: po.recv,
                send: po.send
            }
        }).collect();
        
        self.on_poll_succeed(ctx, poll_results);
    }

    fn on_poll_succeed(&mut self, ctx: &mut Context, poll_results: Vec<PollRes>) {
        if let Some(timeout) = self.timeout.take() {
            ctx.cancel(timeout);
        }
        self.clear_votes();
        self.send_reply(Reply::Poll(poll_results));
    }

    fn clear_votes(&mut self) {
        for i in 0..self.poll_opts.len() {
            self.recv_votes[i] = None;
            self.send_votes[i] = None;
        }

    }

    pub fn get_socket_ids(&self) -> Vec<SocketId> {
        self.poll_opts.iter().map(|po| po.sid).collect()
    }
}

fn is_vote_over(interest: bool, vote: &Option<bool>) -> bool {
    match (interest, *vote) {
        (false, _) => true,
        (true, None) => false,
        (true, Some(x)) => x
    }
}

fn is_socket_side_ready(interest: bool, vote: &Option<bool>) -> bool {
    match (interest, *vote) {
        (false, _) | (true, None) => false,
        (true, Some(x)) => x
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.send_reply(Reply::Closed)
    }
}

/*****************************************************************************/
/*                                                                           */
/* tests                                                                     */
/*                                                                           */
/*****************************************************************************/

#[cfg(test)]
mod tests {

    use std::fmt;
    use std::rc::Rc;
    use std::cell::RefCell;
    use std::io::Result;
    use std::time::Duration;
    use std::sync::mpsc;

    use core::{SocketId, PollReq, Scheduled};

    use super::*;

    #[test]
    fn when_not_ready_event_with_interest_is_received_no_reply_is_sent() {
        let (tx, rx) = mpsc::channel();
        let sid = SocketId::from(1);
        let poll_req = PollReq {
            sid: sid,
            recv: true,
            send: false
        };
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let mut probe = Probe::new(tx, vec![poll_req]);

        probe.poll(&mut ctx, Duration::from_millis(100));
        probe.on_socket_can_recv(&mut ctx, sid, false);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn when_ready_event_without_interest_is_received_no_reply_is_sent() {
        let (tx, rx) = mpsc::channel();
        let sid = SocketId::from(1);
        let poll_req = PollReq {
            sid: sid,
            recv: true,
            send: false
        };
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let mut probe = Probe::new(tx, vec![poll_req]);

        probe.poll(&mut ctx, Duration::from_millis(100));
        probe.on_socket_can_send(&mut ctx, sid, true);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn when_ready_event_with_interest_is_received_reply_is_sent() {
        let (tx, rx) = mpsc::channel();
        let sid = SocketId::from(1);
        let poll_req = PollReq {
            sid: sid,
            recv: true,
            send: false
        };
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let mut probe = Probe::new(tx, vec![poll_req]);

        probe.poll(&mut ctx, Duration::from_millis(100));
        probe.on_socket_can_recv(&mut ctx, sid, true);
        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_poll_res = match reply {
            Reply::Poll(x) => Some(x),
            _ => None
        };
        let poll_res = reply_poll_res.unwrap();
        assert_eq!(1, poll_res.len());
        assert!(poll_res[0].recv);
        assert!(!poll_res[0].send);
    }

    struct TestContextSensor {
        schedule_cancellations: Vec<Scheduled>,
        poll_calls: Vec<SocketId>
    }

    struct TestContext {
        sensor: Rc<RefCell<TestContextSensor>>,
        scheduled_seq: usize
    }

    impl Default for TestContextSensor {
        fn default() -> TestContextSensor {
            TestContextSensor {
                schedule_cancellations: Vec::new(),
                poll_calls: Vec::new()
            }
        }
    }

    impl TestContextSensor {
        fn push_schedule_cancellation(&mut self, scheduled: Scheduled) {
            self.schedule_cancellations.push(scheduled)
        }
        fn push_poll_call(&mut self, sid: SocketId) {
            self.poll_calls.push(sid)
        }
    }

    impl TestContext {
        pub fn with_sensor(sensor: Rc<RefCell<TestContextSensor>>) -> TestContext {
            TestContext {
                sensor: sensor,
                scheduled_seq: 0
            }
        }
    }

    impl fmt::Debug for TestContext {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "TestContext")
        }
    }

    impl Scheduler for TestContext {
        fn schedule(&mut self, _: Schedulable, _: Duration) -> Result<Scheduled> {
            self.scheduled_seq += 1;

            Ok(Scheduled::from(self.scheduled_seq))
        }
        fn cancel(&mut self, scheduled: Scheduled) {
            self.sensor.borrow_mut().push_schedule_cancellation(scheduled)
        }
    }

    impl Context for TestContext {
        fn poll(&mut self, sid: SocketId) {
            self.sensor.borrow_mut().push_poll_call(sid);
        }
    }

}