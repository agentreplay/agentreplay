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

//! Streaming K-way merge for observations.

use flowtrace_core::observation::Observation;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

pub struct KWayMerge<I: Iterator<Item = Observation>> {
    heap: BinaryHeap<Reverse<HeapEntry<I>>>,
}

struct HeapEntry<I: Iterator<Item = Observation>> {
    current: Observation,
    iterator: I,
}

impl<I: Iterator<Item = Observation>> Ord for HeapEntry<I> {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = self.current.created_at.packed();
        let rhs = other.current.created_at.packed();
        lhs.cmp(&rhs)
    }
}

impl<I: Iterator<Item = Observation>> PartialOrd for HeapEntry<I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<I: Iterator<Item = Observation>> PartialEq for HeapEntry<I> {
    fn eq(&self, other: &Self) -> bool {
        self.current.created_at.packed() == other.current.created_at.packed()
    }
}

impl<I: Iterator<Item = Observation>> Eq for HeapEntry<I> {}

impl<I: Iterator<Item = Observation>> KWayMerge<I> {
    pub fn new(iterators: Vec<I>) -> Self {
        let mut heap = BinaryHeap::with_capacity(iterators.len());

        for mut iter in iterators {
            if let Some(current) = iter.next() {
                heap.push(Reverse(HeapEntry { current, iterator: iter }));
            }
        }

        Self { heap }
    }
}

impl<I: Iterator<Item = Observation>> Iterator for KWayMerge<I> {
    type Item = Observation;

    fn next(&mut self) -> Option<Self::Item> {
        let Reverse(mut entry) = self.heap.pop()?;
        let result = entry.current;

        if let Some(next) = entry.iterator.next() {
            entry.current = next;
            self.heap.push(Reverse(entry));
        }

        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.heap.len(), None)
    }
}
