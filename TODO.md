Update rust-closure-playground repo with FnBox findings
Decide what to do with non-blocking recv and send

### Current problem: REQ resend
There can be only one operation in progress for a given socket but resend occurs in background.
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (a some bytes sent, but not all).

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### General:
- write documentation
- adds embedded code example to the front page
- setup documentation generation and site and github pages

### Refactors:  
- Rename session into something less oriented ? (context, environment ...)
- Remove association between token and socketid when a pipe is dead
- Register to readable or writable only when required, that is when operation status is in progress ? 
- maybe the acceptor could create pipes instead of connections ?
- Implement Read & Write trait on sockets
- Use a pool for payloads

### Features:
- For raw bus socket, store the pipe token in the header when receving a message. When sending, check if there is a pipe token in the header and skip the specified pipe
- Check what to do when send/recv timeout is reached and parts of the message has already been transfered !
- Have Socket::connect & bind return an Endpoint that can be shut down
- Implement nanocat
- Expose the event loop configuration ?

### Stuff to look at:
https://github.com/tailhook/rotor  
https://github.com/dpc/mioco  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  

### Idea for device implementation
To implement a device simply, the raw socket facade should support being accessed by two threads
and the protocol should support 1 send & 1 recv operation in //.
This requires a dedicated channel for send and another for recv.
Otherwise the first finished operation would send an event on the unique channel.
Since the notified event would not match the expectations of one of the waiter, it cannot work.

Some protocols do not support send or recv and return an error.
So how could the device function call it safely ?