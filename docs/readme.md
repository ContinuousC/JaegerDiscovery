# Application Discovery from Traces

Jaeger Discovery queries trace data from Opensearch indices written by Jaeger,
analyses which services and operations are currently active and how they are
interconnected, and writes the result to the Relation Graph Engine.

## Mechanism

Application discovery is implemented as a stateful process. This is required to
maintain stable ids for operations and services, and allows to avoid re-querying
data that has already been processed.

When discovery is first started, an empty state will be created. The state
contains the last timestamp processed, a map of discovered services and
operations with relations between them, and a map of in-progress traces and
spans. Discovery is run every minute and updates the state. After discovery is
finished, the updated state is written to disk, ensuring the next run can pick
up where we left off, even in the case of failure.

Spans are queried and processed in a streaming fashion. If no last timestamp is
known (i.e. when discovery is first run or if the state has been deleted), spans
from the last seven days are queried. Otherwise, discovery queries span starting
from the last seen timestamp. We expect spans to be written in order. If this
would show not to be the case, a slight overlap could be applied, re-processing
spans for that period.

For every span, the `trace_info` and its contained `span_info` map are updated.
Apart from the span info map, the trace info contains a `last_seen` timestamp to
allow cleaning up trace data after a set threshold. The span info contains a
list of children identified by key for delayed processing (`parent_of`). This
list is used to allow finding relations between spans that are seen out-of-order
due to clock skew between different services.

For every span, the service key, consisting of the service name, service
namespace and service instance id, is calculated, and the service metadata is
updated in the service map, adding a new service if necessary. The service state
contains an id that is used to identify the service in the Relation Graph.

The service state also contains a map of operations associated with the service.
For every span, the operation state is updated or a new operation is created. As
with the service state, the operation state contains an id used to identify the
operation in the Relation Graph. It also contains a `last_seen` timestamp that
is updated every time a span for the operation is observed.

Subsequently, for every span, relations are updated based on the span's
`ChildOf` relations, containing the parent's trace and span ids. For each of
these relations, the parent span is looked up in the trace and span info map. If
the parent span is found (that is, if the `SpanInfo` entry has its `key`field
set), the relation is processed immediately, updating the service and operation
map. If the parent span is not found, it may still arrive later due to clock
skew. To support this case, an empty span info structure is inserted and the
current span's key is added to the `parent_of` list.

Finally, the current span's `parent_of` list, containing data on child spans
seen before the current span was observed, is processed, updating relation info
in the service and operations map.

After each chunk of spans received from the database, the trace and span map is
cleaned up, removing info on traces not seen in the last five minutes. Relations
between services with a clok skew higher than this threshold, will not be
detected.

When the query is finished, the service and operation map is cleaned up,
removing any services and operations not seen in the last seven days. This
threshold determines when services and operations are considered to be no longer
in existence and can be removed from the Relation Graph.

Then, with all spans processed and the state updated, a map of items and
relations is built from the services and operations state and sent to the
Relation Graph Engine. The state is then committed to disk, and the Jaeger
Discovery daemon sleeps until the next discovery is due.
