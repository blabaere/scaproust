### General:
 - write documentation
 - adds embedded code example to the front page
 - setup documentation generation and site and github pages
 - setup CI with appveyor once mio is compatible with windows

### Current problem:  
If no pipe is available when sending or receiving, an error is returned.
The expected behavior is to wait the timeout for a pipe to be added (via reconnect or accept).
To be able to do that, it would require to move the pending message from proto-pipe to protocol.
Each proto-pipe would then keep the progress status.
In that case, status should be: sent, sending, postponed, failed, or none if no operation is in progress.
When a proto-pipe, become ready and any pending operation should be handed to that proto-pipe.
When the operation is finished, each proto-pipe is notified of the operation completion.

For protocol that are sent to single, when a proto pipe status becomes sending or receiving,
the other proto-pipes should have their current operation cancelled.

### Next problem:
There is too much code duplication in the protocol part.
Refactor the proto pipes with some traits to share more code.
Traits could be:
 - SenderPipe
 - ReceiverPipe
 - TwoWayPipe

Refactoring the protocols may be harder but there are some crosscutting aspects:
 - Send to single (pair, push, req, rep, resp)
 - Send to many (bus, pub, surv)
 - No sending (pull, sub)

Some header related code could also be shared:
 - Message with id (req/rep, surv/resp)
 - Send back to originator only (req/rep, surv/resp)
 

### Refactors:  
 - Rename session into something less oriented ? (context, environment ...)
 - Remove association between token and socketid when a pipe is dead
 - Register to readable or writable only when required, that is when operation status is in progress ? 
 - find a better name for socket_impl and session_impl
 - maybe the acceptor could create pipes instead of connections ?
 - Implement Read & Write trait on sockets
 - Use a pool for payloads

### Features:
 - For raw bus socket, store the pipe token in the header when receving a message  
   When sending, check if there is a pipe token in the header and skip the specified pipe
 - Check what to do when send/recv timeout is reached and parts of the message has already been transfered !
 - Now that there is send & receive, connect & bind : TEST ALL THE THINGS !!!
 - Have Socket::connect & bind return an Endpoint that can be shut down
 - Implement nanocat
 - Expose the event loop configuration ?


### Stuff to look at:
https://github.com/dpc/mioco  
https://github.com/dwrensha/gj  
https://github.com/calc0000/tcp-loop  
https://github.com/zonyitoo/simplesched  
