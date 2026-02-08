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

//! Count-Min Sketch for probabilistic duplicate counting.
//!
//! A fixed-memory data structure for tracking approximate counts without
//! the unbounded memory growth of HashMap.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// Count-Min Sketch parameters
const DEPTH: usize = 4; // Number of hash functions
const WIDTH: usize = 16384; // Width of each row (2^14)

/// A thread-safe Count-Min Sketch for approximate frequency counting.
///
/// Memory usage is fixed at DEPTH * WIDTH * 8 bytes = 512KB regardless
/// of how many unique items are counted.
pub struct CountMinSketch {
    /// 2D array of counters: [depth][width]
    counters: Vec<Vec<AtomicU64>>,
    /// Seeds for hash functions
    seeds: [u64; DEPTH],
}

impl CountMinSketch {
    /// Create a new Count-Min Sketch with fixed memory footprint.
    pub fn new() -> Self {
        let mut counters = Vec::with_capacity(DEPTH);
        for _ in 0..DEPTH {
            let mut row = Vec::with_capacity(WIDTH);
            for _ in 0..WIDTH {
                row.push(AtomicU64::new(0));
            }
            counters.push(row);
        }

        Self {
            counters,
            seeds: [
                0x517cc1b727220a95,
                0x2545f4914f6cdd1d,
                0xc4ceb9fe1a85ec53,
                0xff51afd7ed558ccd,
            ],
        }
    }

    /// Increment the count for an item and return the new estimated count.
    pub fn increment(&self, item: u128) -> u64 {
        let mut min_count = u64::MAX;

        for (i, row) in self.counters.iter().enumerate() {
            let index = self.hash(item, self.seeds[i]);
            let new_count = row[index].fetch_add(1, Ordering::Relaxed) + 1;
            min_count = min_count.min(new_count);
        }

        min_count
    }

    /// Get the estimated count for an item (minimum across all hash functions).
    #[allow(dead_code)]
    pub fn estimate(&self, item: u128) -> u64 {
        let mut min_count = u64::MAX;

        for (i, row) in self.counters.iter().enumerate() {
            let index = self.hash(item, self.seeds[i]);
            let count = row[index].load(Ordering::Relaxed);
            min_count = min_count.min(count);
        }

        min_count
    }

    /// Reset all counters (useful for periodic decay).
    #[allow(dead_code)]
    pub fn reset(&self) {
        for row in &self.counters {
            for counter in row {
                counter.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Decay all counters by dividing by 2 (useful for time-windowed counting).
    #[allow(dead_code)]
    pub fn decay(&self) {
        for row in &self.counters {
            for counter in row {
                let old = counter.load(Ordering::Relaxed);
                counter.store(old / 2, Ordering::Relaxed);
            }
        }
    }

    /// Hash a u128 item to a column index.
    fn hash(&self, item: u128, seed: u64) -> usize {
        let mut hasher = SimpleHasher::new(seed);
        item.hash(&mut hasher);
        (hasher.finish() as usize) % WIDTH
    }
}

impl Default for CountMinSketch {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple fast hasher for Count-Min Sketch.
struct SimpleHasher {
    state: u64,
}

impl SimpleHasher {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Hasher for SimpleHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state = self.state.wrapping_mul(0x5851f42d4c957f2d);
            self.state ^= *byte as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_min_sketch() {
        let sketch = CountMinSketch::new();

        // Increment same item multiple times
        assert_eq!(sketch.increment(123), 1);
        assert_eq!(sketch.increment(123), 2);
        assert_eq!(sketch.increment(123), 3);

        // Estimate should match
        assert_eq!(sketch.estimate(123), 3);

        // New item starts at 1
        assert_eq!(sketch.increment(456), 1);
    }

    #[test]
    fn test_decay() {
        let sketch = CountMinSketch::new();

        for _ in 0..100 {
            sketch.increment(999);
        }

        assert_eq!(sketch.estimate(999), 100);
        sketch.decay();
        assert_eq!(sketch.estimate(999), 50);
    }
}
