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

//! Streaming K-way merge for observations.

use agentreplay_core::observation::Observation;
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
