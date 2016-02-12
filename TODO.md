### Current problem: REQ resend
There can be only one operation in progress for a given socket but resend occurs in background.
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (a some bytes sent, but not all).

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Lots of renaming

#### SocketEvtSignal::Connected : 
Sent by a socket to the session to tell that a pipe has been added to a socket,
so it can setup the socket <-> pipe mapping.
Then the session fowards the evt signal to the socket.
SHOULD BE CALLED ON_PIPE_ADDED
AND THE MISSING ON_PIPE_REMOVED COUNTERPART SHOULD BE ADDED

RENAME socket.handle_evt and avoid reusing the EvtSignal enum

##### Protocol::register_pipe
Used by a socket to tell the protocol the pipe can now be plugged in the IO evt system
 and perform the handshake.
MAYBE OPEN_PIPE
##### PipeEvtSignal::Opened
Sent by a pipe to tell the socket that the handshake his done and it can be used to send and receive
CAN STAY LIKE THIS IF THE PREVIOUS IS MENTIONING 'OPEN'
##### Protocol::on_pipe_register
Used by a socket to tell the protocol that the handshake his done ...
ON_PIPE_OPENED

### Device
 - Devices should be created by the session and implement a 'Runnable' trait
 - Poll !!!

### Next problems
- Handle accept error
- Have pipe error forwarded to the session and the socket
- Session shutdown should unblock current socket operations

### General:
- write documentation
- adds embedded code example to the front page
- setup documentation generation and site and github pages

### Refactors:
- Rename session into something less oriented ? (context, environment ...)
- Implement Read & Write trait on sockets
- Use a pool for payloads and buffers (if any)

### Features:
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

### Windows problem: non-blocking send not available
The way mio works on Windows currently makes it impossible to send several chunks of bytes
without having to wait for an event loop round-trip.
This is required for dist based protocols to work (pub, survey and bus).
The applied workaround is to create a buffer for each non-blocking send operation.
