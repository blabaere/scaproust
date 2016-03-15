### Send & recv timeout handling seems incorrect
When a timeout is reached while a pipe operation is already started,
is it really a good idea to pretend nothing happened ?
It can happen when the timeout is too aggressive and the remote end is busy (due to peak load?).
This would probably end up with the remote end in a state where it is expecting the rest of the operation to complete.  
  
In nanomsg it seems that when a send or recv is not completed right away, the pipe is removed from the 
fair queue / load balancer and added back only once the operation is completed.  
  
In this case, cancel would just tell the protocol not to notify the facade of the operation completion.
But there is still a need to add back the pipe to the fq/lb once a readiness occurs AFTER the completion.  
  
Actions:
1. Store sending/receiving pipe id in protocol.
- Stop using the priolist::current to know which pipe is sending or receiving.
- Move that information inside the protocol state.

2. Make the sending and receiving states of a pipe independant.  
- Merge all the live pipe state into a single one.
- Create a sending side and receiving side pipe state.
- Have the live pipe state owns one state for each side.

3. Enhance the priolist item with a visibility field.  
- Have the pipe tell the protocol that sending or receiving is blocked, waiting on the remote end.
- Add show/hide methods to priolist, a pipe can only be current if it is both active and visible.
- Use this to add the pipe to the priolist earlier, but with hidden visibility.
- Have the protocol show the pipe when the open ack notification is received.
- When a pipe operation is blocked, hide AND deactivate it.
- When the operation finally completes, show the pipe, but wait for readiness to activate it.
- Remove the cancel_send and cancel_recv methods from pipe.

### When the sending/receiving pipe is removed, it probably means the operation has failed

### REQ resend cannot be implemented with the current design
There can be only one operation in progress for a given socket but resend occurs in background.
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (some bytes sent, but not all).

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Receving a malformed message will cause the recv operation to end in timeout
When a protocol receives a "malformed" message, the message is dropped, but the facade is not notified of anything and no pipe is asked to recv again

### Next problems
- Add some doc comment on each SocketType variant
- Create a core folder, for socket, acceptor and session modules
- Create a facade folder for modules named *_facade
- Create an io (or something better ?) for pipe, send and recv modules
- Have pipe error forwarded to the session and the socket
- Handle accept error

### Refactors:
- Rename session into something less oriented ? (context, environment ...)
- Implement Read & Write trait on sockets
- Use a pool for payloads and buffers (if any)

### Features:
- Implement nanocat
- Expose the event loop configuration ?

### Stuff to look at:
**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  

https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  

### Windows problem: non-blocking send not available
The way mio works on Windows currently makes it impossible to send several chunks of bytes
without having to wait for an event loop round-trip.
This is required for dist based protocols to work (pub, survey and bus).
The applied workaround is to create a buffer for each non-blocking send operation.
