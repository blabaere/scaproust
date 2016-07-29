### Fix fairqueue and load balancing design
The I/O readiness notifications belong to the transport layer and should only be used there, by the pipe. Especially, the socket/protocol should not do anything more than forwarding it.
Instead the pipe should publish sendable/receivable notifications that would be slightly different from writable/readable since they would depend on pipe state and operation progress.
For example a readable notification during handshake or during a recv operation would not raise a receivable notification.

Maybe the protocol should not own the pipe but rather send command to them via the event loop.
It could also make the Activable state useless.

### 'readable' + 'hup' inside a loop with handshake and message  in the buffer
This goes wrong, probably because the resubscription fails due to hup.
It could be fixed by recv prefetch ?
Anyway, not listening to hup is probably not a good idea since 
it would allow fair queue to select a gone peer

### recv pre fetch
This needs to be controlled at the protocol level.
Otherwise we would prefetch at most 'n' messages instead of just one.
So when the protocol sees a pipe becoming active and readable,
it should check if it does not have a pending received message
and then start the recv pipe operation.
This should be done in a new state: prefetch.
When a recv request is received while in prefetch, switch to receiving
When a recv request is received while in Idle state with a pending msg,
it should be delivered right away.

### use macros for factorizing common protocol code ?

### define a struct wrapping the event loop reference and the notification token
There is too much coupling between the protocol and the event loop.
Stuff like registration and timeout should be exposed by that struct.

### REQ resend 
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (some bytes sent, but not all) ?

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Receving a malformed message will cause the recv operation to end in timeout
When a protocol receives a "malformed" message, the message is dropped, but the facade is not notified of anything and no pipe is asked to recv again

### Next tasks
- LINGER !!!
- Handle accept error

### AUTOMATE ALL THE THINGS !!!

Test reconnect feature

### Features:
- Implement nanocat
- STAR protocol ?

### Performance
- Use a pool for payloads and buffers (if any)

### change copyright header to mention 'authors' and the AUTHORS file

### Stuff to look at:

https://github.com/frankmcsherry/recycler
https://github.com/zslayton/lifeguard
http://carllerche.github.io/pool/pool/

**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  

https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  
