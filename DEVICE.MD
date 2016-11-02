This document explains how a device can work with stateful protocols like REQ/REP.
It shows the content of the message at each step of the workflow.
The message is represent as follow: `([header][body])`

| Step | Message | Comment |
| --- | --- | --- |
| APP | ([][req-body]) | The application creates a request and send it via a REQ socket |
| Send | ([req-id][req-body]) | The REQ socket generates a req-id and appends it to the message header. The REQ socket now has a pending request in its state. |
| Send | ([][req-id;req-body]) | The message is sent from one endpoint to another over the transport |
| Recv | ([eid1][req-id;req-body]) | The device REP socket appends the receving endpoint id the message header |
| Recv | ([eid1;req-id][req-body]) | The device REP socket moves 4 bytes from the body to the header |
| FWD | ([eid1;req-id][req-body]) | The device forwards the untouched message to its REQ socket |
| Send | ([eid1;req-id][req-body]) | The device REQ socket DOES NOT generate a req-id or append anything |
| Send | ([][eid1;req-id;req-body]) | The message is sent from one endpoint to another over the transport |
| Recv | ([eid2][eid1;req-id;req-body]) | The REP socket appends the receving endpoint id the message header |
| Recv | ([eid2;eid1;req-id][req-body]) | The REP socket moves the message backtrace from the body to the header |
| Recv | ([][req-body]) | The REP socket moves the message header and stores it in its state |
| APP |  | At this point the application is given a message with the original request, the device sockets don't remember anything while the 'outer' REQ & REP have kept a trace of the request flow |
| Send | ([][rep-body]) | The application creates a reply and send it via a REP socket |
| Send | ([eid2;eid1;req-id][rep-body]) | The REP socket moves the saved backtrace from its state to the message header |
| Send | ([eid1;req-id][rep-body]) | The REP socket removes 4 bytes from the header to get an endpoint id |
| Send | ([][eid1;req-id;rep-body]) | The message is sent from one endpoint to another over the transport |
| Recv | ([eid1][req-id;rep-body]) | The device REQ socket moves 4 bytes from the body to the header |
| Recv | ([eid1][req-id;rep-body]) | The device REQ socket does no additional check and does not change its state further |
| FWD | ([eid1][req-id;rep-body]) | The device forwards the untouched message to its REP socket |
| Send | ([eid1][req-id;rep-body]) | The device REP socket does not fiddle with any backtrace, it does not have one |
| Send | ([][req-id;rep-body]) | The device REP socket removes 4 bytes from the header to get an endpoint id |
| Send | ([][req-id;rep-body]) | The message is sent from one endpoint to another over the transport |
| Recv | ([req-id][rep-body]) | The REQ socket moves 4 bytes from the body to the header |
| Recv | ([req-id][rep-body]) | The REQ socket checks the header req id against the pending one in its state |
| APP | ([req-id][rep-body]) | At this point the application is given a message with the original request id and the reply |
