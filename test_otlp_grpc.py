#!/usr/bin/env python3
"""Quick test: send a span via gRPC OTLP to localhost:47117"""
import grpc
import time
import os

from opentelemetry.proto.collector.trace.v1 import trace_service_pb2_grpc, trace_service_pb2
from opentelemetry.proto.trace.v1 import trace_pb2
from opentelemetry.proto.common.v1 import common_pb2
from opentelemetry.proto.resource.v1 import resource_pb2

channel = grpc.insecure_channel('localhost:47117')
stub = trace_service_pb2_grpc.TraceServiceStub(channel)

resource = resource_pb2.Resource(attributes=[
    common_pb2.KeyValue(key='service.name', value=common_pb2.AnyValue(string_value='grpc-test')),
    common_pb2.KeyValue(key='project_id', value=common_pb2.AnyValue(int_value=40032)),
    common_pb2.KeyValue(key='tenant_id', value=common_pb2.AnyValue(int_value=1)),
])

now_ns = int(time.time() * 1e9)

span = trace_pb2.Span(
    trace_id=os.urandom(16),
    span_id=os.urandom(8),
    name='test-grpc-span',
    kind=trace_pb2.Span.SPAN_KIND_INTERNAL,
    start_time_unix_nano=now_ns,
    end_time_unix_nano=now_ns + 1000000000,
)

request = trace_service_pb2.ExportTraceServiceRequest(
    resource_spans=[trace_pb2.ResourceSpans(
        resource=resource,
        scope_spans=[trace_pb2.ScopeSpans(spans=[span])]
    )]
)

try:
    response = stub.Export(request, timeout=5)
    print(f'SUCCESS: {response}')
except grpc.RpcError as e:
    print(f'gRPC ERROR: {e.code()} - {e.details()}')
except Exception as e:
    print(f'ERROR: {type(e).__name__}: {e}')
