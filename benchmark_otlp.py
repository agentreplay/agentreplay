#!/usr/bin/env python3
"""
Benchmark OTLP gRPC Ingestion
Generates synthetic traces using Faker and sends them via gRPC.
Configuration based on user request:
- Service: j45
- Project: 29676
- Tenant: 1
- Endpoint: localhost:47117
"""
import grpc
import time
import os
import argparse
import random
import threading
import sys
from concurrent.futures import ThreadPoolExecutor
from faker import Faker

# Ensure opentelemetry-proto is installed
try:
    from opentelemetry.proto.collector.trace.v1 import trace_service_pb2_grpc, trace_service_pb2
    from opentelemetry.proto.trace.v1 import trace_pb2
    from opentelemetry.proto.common.v1 import common_pb2
    from opentelemetry.proto.resource.v1 import resource_pb2
except ImportError:
    print("Error: opentelemetry-proto not installed. Run: pip install opentelemetry-proto")
    sys.exit(1)

fake = Faker()

# Configuration Constants
SERVICE_NAME = os.getenv("AGENTREPLAY_SERVICE_NAME", "test45")
PROJECT_ID = int(os.getenv("AGENTREPLAY_PROJECT_ID", "6861"))
TENANT_ID = int(os.getenv("AGENTREPLAY_TENANT_ID", "1"))
ENDPOINT = os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:47117")

# Sample data pools to optimize Faker usage
MODELS = ["gpt-4-turbo", "gpt-3.5-turbo", "claude-3-opus", "gemini-1.5-pro", "llama-3-70b"]
ROLES = ["system", "user", "assistant"]

# Pre-generate data pools to avoid Faker CPU overhead in the hot loop
print("Pre-generating data pools...")
SAMPLE_PROMPTS = [fake.sentence(nb_words=10) for _ in range(1000)]
SAMPLE_COMPLETIONS = [fake.paragraph(nb_sentences=3) for _ in range(1000)]
print("Data pools ready.")

def create_resource():
    return resource_pb2.Resource(attributes=[
        common_pb2.KeyValue(key='service.name', value=common_pb2.AnyValue(string_value=SERVICE_NAME)),
        common_pb2.KeyValue(key='project_id', value=common_pb2.AnyValue(int_value=PROJECT_ID)),
        common_pb2.KeyValue(key='tenant_id', value=common_pb2.AnyValue(int_value=TENANT_ID)),
        common_pb2.KeyValue(key='telemetry.sdk.language', value=common_pb2.AnyValue(string_value='python')),
        common_pb2.KeyValue(key='agentreplay.enabled', value=common_pb2.AnyValue(string_value="true")),
    ])

def generate_span(trace_id=None):
    if not trace_id:
        trace_id = os.urandom(16)
    span_id = os.urandom(8)
    now_ns = int(time.time() * 1e9)
    # Random duration between 50ms and 2s
    duration_ns = random.randint(50, 2000) * 1_000_000 
    
    prompt = random.choice(SAMPLE_PROMPTS)
    completion = random.choice(SAMPLE_COMPLETIONS)
    
    prompt_len = len(prompt)
    completion_len = len(completion)
    
    attributes = [
        common_pb2.KeyValue(key='gen_ai.system', value=common_pb2.AnyValue(string_value='openai')),
        common_pb2.KeyValue(key='gen_ai.request.model', value=common_pb2.AnyValue(string_value=random.choice(MODELS))),
        common_pb2.KeyValue(key='gen_ai.usage.input_tokens', value=common_pb2.AnyValue(int_value=prompt_len)),
        common_pb2.KeyValue(key='gen_ai.usage.output_tokens', value=common_pb2.AnyValue(int_value=completion_len)),
        common_pb2.KeyValue(key='gen_ai.usage.total_tokens', value=common_pb2.AnyValue(int_value=prompt_len + completion_len)),
        # Standard attributes
        common_pb2.KeyValue(key='span.kind', value=common_pb2.AnyValue(string_value='client')),
        # Prompt (simulated)
        common_pb2.KeyValue(key='gen_ai.prompt.0.role', value=common_pb2.AnyValue(string_value='user')),
        common_pb2.KeyValue(key='gen_ai.prompt.0.content', value=common_pb2.AnyValue(string_value=prompt)),
        # Completion (simulated)
        common_pb2.KeyValue(key='gen_ai.completion.0.role', value=common_pb2.AnyValue(string_value='assistant')),
        common_pb2.KeyValue(key='gen_ai.completion.0.content', value=common_pb2.AnyValue(string_value=completion)),
    ]

    return trace_pb2.Span(
        trace_id=trace_id,
        span_id=span_id,
        name='openai.chat',
        kind=trace_pb2.Span.SPAN_KIND_CLIENT,
        start_time_unix_nano=now_ns,
        end_time_unix_nano=now_ns + duration_ns,
        attributes=attributes
    )

def worker(args, shared_stats):
    try:
        channel = grpc.insecure_channel(args.target)
        stub = trace_service_pb2_grpc.TraceServiceStub(channel)
        resource = create_resource()
        
        while not args.stop_event.is_set():
            # Check if we reached the target
            with args.lock:
                if args.total_spans >= args.max_traces:
                    break
                
            batch_size = args.batch_size
            spans = [generate_span() for _ in range(batch_size)]
            
            request = trace_service_pb2.ExportTraceServiceRequest(
                resource_spans=[trace_pb2.ResourceSpans(
                    resource=resource,
                    scope_spans=[trace_pb2.ScopeSpans(spans=spans)]
                )]
            )
            
            try:
                stub.Export(request, timeout=5)
                with args.lock:
                    args.total_spans += batch_size
            except Exception as e:
                # print(f"Error: {e}") # Suppress individual error logs for speed
                pass
                
        channel.close()
    except Exception as e:
        print(f"Worker init failed: {e}")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", default=ENDPOINT, help="OTLP gRPC target")
    parser.add_argument("--concurrency", type=int, default=10, help="Thread count")
    parser.add_argument("--batch-size", type=int, default=1000, help="Spans per batch")
    parser.add_argument("--max-traces", type=int, default=2000000, help="Total traces")
    args = parser.parse_args()

    print(f"🚀 Benchmarking Agentreplay Ingestion")
    print(f"Target: {args.target}")
    print(f"Config: Service={SERVICE_NAME}, Project={PROJECT_ID}, Tenant={TENANT_ID}")
    print(f"Goal: {args.max_traces} traces using {args.concurrency} threads")
    
    args.stop_event = threading.Event()
    args.lock = threading.Lock()
    args.total_spans = 0
    
    start_time = time.time()
    
    with ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        futures = [executor.submit(worker, args, None) for _ in range(args.concurrency)]
        
        try:
            while args.total_spans < args.max_traces:
                time.sleep(1)
                elapsed = time.time() - start_time
                count = args.total_spans
                rate = count / elapsed if elapsed > 0 else 0
                print(f"Progress: {count:,} / {args.max_traces:,} | Rate: {rate:.0f} spans/sec | Elapsed: {elapsed:.0f}s", end='\r')
                
                if count >= args.max_traces:
                    break
        except KeyboardInterrupt:
            print("\nStopping...")
            args.stop_event.set()
    
    total_time = time.time() - start_time
    print(f"\n\n✅ Benchmark Complete")
    print(f"Total Spans: {args.total_spans:,}")
    print(f"Total Time: {total_time:.2f}s")
    print(f"Avg Throughput: {args.total_spans / total_time:.0f} spans/sec")

if __name__ == "__main__":
    main()
