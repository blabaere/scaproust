To fix device bug and implement client side poll, socket should expose a `poll` function.
And delegate the call to the protocol which will raise CanSend or CanRecv events accordingly.

PB: device needs to wait until one of the sockets becomes readable, 
while poll needs to know for each socket if it's readable or writable within the timeout.

Bug scenario:
While not checking, a device receives two CanRecv notifications.
Then it is checked, the reply is sent immediatly.
When checked again, the socket is not seen as 'CanRecv'.
Now the device can wait forever.
There is nothing that can be done at the device level, so the socket/protocol must raise 'CanRecv' right after receiving if it is still able to recv.
And it is probably the same for CanSend.

Poll design:
 - Currently core::context::Event::CanSend is never raised, the protocols must be raising it before implementing poll.
 - Polling requires the front-end to pass a variable number of socket ids to the back-end. This means heap allocation on each call, if this proves problematic, a poller struct could be created on each side. Maybe an Arc could be exchanged back and forth between each side.
 - When a poll is in progress, the polled sockets should not be used. One way to prevent it is to borrow the sockets for the duration of poll.
 - On the back-end side a probe should listen to readable/writable events raised by sockets. This is very similar to the way bridge device currently works so there is probably something to be shared. In the same way, the readable/writable value must be stored between polls.
 - A when the poll timeout is reached, the reply should be sent to the front-end.
 - What if the poller is created after the socket is connected ? The writable events have already been published, and the poller will not receive them ...

### Improvements
- Reconnect interval max 
- Linger
- Handle accept error
- Req prefetch replies
- Use a pool for payloads and buffers (if any)
- Find something more efficient than a channel for sending replies from the event loop back to the facade (a mailbox?)
- IPC transport on windows : See https://github.com/mmacedoeu/pipetoredis.rs
- INPROC transport : to be determined (rust channel's are probably doing a better work at this)

### Features
- Non-blocking versions of send and recv
- Websocket transport
- TLS transport
- Implement nanocat
- STAR protocol ?
- Polling

### Vision
- Expose async io using future-rs ?

### Tasks
- Use github issues instead of this file
- Document release process
- Document contribution mode
- AUTOMATE ALL THE THINGS !!! (compat test, benchmark ...)
- Change copyright header to mention 'authors' and the AUTHORS file
- Adds coverage to the build and display it


### Things to look at

gather/scatter io operations
https://github.com/seanmonstar/vecio

Windows named pipes
https://github.com/mmacedoeu/pipetoredis.rs

https://pascalhertleif.de/artikel/good-practices-for-writing-rust-libraries/
http://keepachangelog.com

https://github.com/tokio-rs
**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  


https://github.com/frankmcsherry/recycler
https://github.com/zslayton/lifeguard
http://carllerche.github.io/pool/pool/


https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  


Websocket
https://github.com/housleyjk/ws-rs  
https://github.com/cyderize/rust-websocket  
