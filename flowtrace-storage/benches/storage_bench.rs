// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use flowtrace_core::{AgentFlowEdge, SpanType};
use flowtrace_storage::LSMTree;
use tempfile::tempdir;
use tokio::runtime::Runtime;

fn bench_write_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("write_throughput");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let dir = tempdir().unwrap();
                let lsm = LSMTree::open(dir.path()).unwrap();

                for i in 0..size {
                    let edge = AgentFlowEdge::new(1, 0, i as u64, i as u64, SpanType::Root, 0);
                    rt.block_on(lsm.put(black_box(edge))).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_point_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let lsm = LSMTree::open(dir.path()).unwrap();

    // Populate with data
    let mut edges = Vec::new();
    for i in 0..10000 {
        let edge = AgentFlowEdge::new(1, 0, i, i, SpanType::Root, 0);
        rt.block_on(lsm.put(edge)).unwrap();
        edges.push(edge);
    }

    c.bench_function("point_query", |b| {
        b.iter(|| {
            let idx = black_box(5000);
            lsm.get(edges[idx].edge_id).unwrap();
        });
    });
}

fn bench_range_scan(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("range_scan");

    let dir = tempdir().unwrap();
    let lsm = LSMTree::open(dir.path()).unwrap();

    // Populate with data
    for i in 0..10000u64 {
        let mut edge = AgentFlowEdge::new(1, 0, i, i, SpanType::Root, 0);
        edge.timestamp_us = i * 1000;
        edge.checksum = edge.compute_checksum();
        rt.block_on(lsm.put(edge)).unwrap();
    }

    for range_size in [100u64, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*range_size));
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            range_size,
            |b, &size| {
                b.iter(|| {
                    lsm.range_scan(black_box(0), black_box(size * 1000))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_edge_serialization(c: &mut Criterion) {
    let edge = AgentFlowEdge::new(1, 0, 1, 100, SpanType::Planning, 0);

    c.bench_function("edge_serialize", |b| {
        b.iter(|| {
            black_box(edge.to_bytes());
        });
    });

    let bytes = edge.to_bytes();

    c.bench_function("edge_deserialize", |b| {
        b.iter(|| {
            AgentFlowEdge::from_bytes(black_box(&bytes)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_write_throughput,
    bench_point_query,
    bench_range_scan,
    bench_edge_serialization
);

criterion_main!(benches);
