// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use agentreplay_core::{AgentFlowEdge, SpanType};
use agentreplay_storage::LSMTree;
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
