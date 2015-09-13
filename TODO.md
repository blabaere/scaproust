Update rust-closure-playground repo with FnBox findings
Decide what to do with non-blocking recv and send

### Current problem:
To implement request resend, a timeout is scheduled every x seconds.
First, this timeout should be scheduled only AFTER the message is actually sent,
otherwise a socket could have both a pending send and resend.
This means someone needs to know when the msg is sent, and store a copy.
When the reply is received or when another request is sent, the timeout should be cancelled.
The problem here is have the receiving part tell the sending part to cancel the timer.
When the timeout is reached, the copy must be grabbed and

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### General:
- write documentation
- adds embedded code example to the front page
- setup documentation generation and site and github pages
- setup CI with appveyor once mio is compatible with windows

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
https://github.com/dpc/mioco  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  
https://github.com/frankmcsherry/abomonation  

### Idea for device implementation
To implement a device simply, the raw socket facade should support being accessed by two threads
and the protocol should support 1 send & 1 recv operation in //.
This requires a dedicated channel for send and another for recv.
Otherwise the first finished operation would send an event on the unique channel.
Since the notified event would not match the expectations of one of the waiter, it cannot work.