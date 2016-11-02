This document explains how a device can work with stateful protocols like REQ/REP.
It shows the content of the message at each step of the workflow.
The message is represent as follow: `([header][body])`

| Step | Message | Comment |
| --- | --- | --- |
|  | ([][req-body]) | The application creates a request and sent it via a REQ socket |
|  | ([req-id][req-body]) | The REQ socket generates a req-id and appends it to the message header. The REQ socket now has a pending request in its state. |
|  | ([][req-id;req-body]) | The message is sent from one endpoint to another over the transport |
|  | ([eid1][req-id;req-body]) | The device REP socket appends the receving endpoint id the message header |
|  | ([eid1;req-id][req-body]) | The device REP socket moves 4 bytes from the body to the header |
|  | ([eid1;req-id][req-body]) | The device forwards the message untouched to its REQ socket |
|  | ([eid1;req-id][req-body]) | The device REQ socket DOES NOT generate a req-id or append anything |
|  | ([][eid1;req-id;req-body]) | The message is sent from one endpoint to another over the transport |
|  | ([eid2][eid1;req-id;req-body]) | The REP socket appends the receving endpoint id the message header |
|  | ([eid2;eid1;req-id][req-body]) | The REP socket moves the message backtrace from the body to the header |
|  | ([][req-body]) | The REP socket moves the message header and stores it in its state |
|  |  | At this point the application is given a message with the original request, the device sockets don't remember anything while the 'outer' REQ & REP have kept a trace of the request flow |

User -> 
([][req-body])
REQ::send, pending=rid ->  // REQ generates a rid, appends it to the header, saves it as pending_rid
([rid][req-body])
NETWORK ->
([][rid;req-body])
D-REP::recv ->       // D-REP appends the eid to the header
([eid1][rid;req-body])
D-REP::recv ->       // D-REP moves the msg's head from body to the header
([eid1;rid][req-body])
D-REP::recv ->       // D-REP does NOT store the header as a backtrace, because it is RAW
([eid1;rid][req-body])
DEVICE ->            // DEVICE forwards it untouched, that is with a header
([eid1;rid][req-body])
D-REQ::send ->       // D-REQ does NOT generate a rid or store anything, because it is RAW
NETWORK ->
([][eid1;rid;req-body])
REP::recv ->         // REP appends the eid to the header
([eid2][eid1;rid;req-body])
REP::recv ->         // REP moves the msg's head from body to the header
([eid2;eid1;rid][req-body])
REP::recv, bt=eid2;eid1;rid ->       // REP moves the header to its own backtrace, because it is NOT RAW
([][req-body])
User -> 
HERE THE REP USER CODE IS PRESENTED AN UNTOUCHED req-body
AND NOW IT WILL SEND A REPLY rep-body
User ->
([][rep-body])
REP::send ->         // REP moves its backtrace to the message header, because it is NOT RAW
([eid2;eid1;rid][rep-body])
REP::send ->         // REP removes 4 bytes from header to find an endpoint
([eid1;rid][rep-body])
NETWORK ->
([][eid1;rid;rep-body])
D-REQ::recv ->       // D-REQ moves 4 bytes from the body to the header
([eid1][rid;rep-body])
D-REQ::recv ->       // D-REQ does not check the pending rid, because it is RAW
([eid1][rid;rep-body])
DEVICE ->            // DEVICE forwards it untouched, that is with a header
([eid1][rid;rep-body])
D-REP::send ->       // D-REP does nothing with its backtrace, because it is RAW
([eid1][rid;rep-body])
D-REP::send ->       // D-REP removes 4 bytes from header to find an endpoint
([][rid;rep-body])
NETWORK ->
([][rid;rep-body])
REQ::recv ->         // REQ moves 4 bytes from the body to the header
([rid][rep-body])
REQ::recv ->         // REQ checks the header against its own pending_rid
([rid][rep-body])
