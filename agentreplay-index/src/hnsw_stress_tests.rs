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

#[cfg(test)]
mod stress_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    
    #[test]
    #[ignore] // Stress test - run manually with `cargo test -- --ignored`
    fn test_concurrent_insert_stress() {
        let config = HnswConfig::default();
        let index = Arc::new(HnswIndex::new(128, config));
        let num_threads = 8;
        let vectors_per_thread = 1_000;
        
        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let index = Arc::clone(&index);
                thread::spawn(move || {
                    for i in 0..vectors_per_thread {
                        let id = (thread_id * vectors_per_thread + i) as u128;
                        let mut vector = vec![0.0; 128];
                        vector[0] = (id as f32).sin();
                        vector[1] = (id as f32).cos();
                        index.insert(id, vector).expect("Insert failed");
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify all vectors were inserted
        let stats = index.stats();
        assert_eq!(stats.num_nodes, (num_threads * vectors_per_thread) as u64);
    }
    
    #[test]
    #[ignore] // Stress test - run manually
    fn test_concurrent_mixed_workload_stress() {
        let config = HnswConfig::default();
        let index = Arc::new(HnswIndex::new(128, config));
        
        // Pre-populate with some vectors
        for i in 0..5_000 {
            let mut vector = vec![0.0; 128];
            vector[0] = (i as f32).sin();
            vector[1] = (i as f32).cos();
            index.insert(i, vector).unwrap();
        }
        
        let num_threads = 16;
        let operations_per_thread = 500;
        
        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let index = Arc::clone(&index);
                thread::spawn(move || {
                    for i in 0..operations_per_thread {
                        if i % 3 == 0 {
                            // Insert
                            let id = (5_000 + thread_id * operations_per_thread + i) as u128;
                            let mut vector = vec![0.0; 128];
                            vector[0] = (id as f32).sin();
                            vector[1] = (id as f32).cos();
                            index.insert(id, vector).expect("Insert failed");
                        } else {
                            // Search
                            let mut query = vec![0.0; 128];
                            query[0] = (thread_id as f32).sin();
                            query[1] = (thread_id as f32).cos();
                            index.search(&query, 10).expect("Search failed");
                        }
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify index is still functional
        let query = vec![1.0; 128];
        let results = index.search(&query, 10).unwrap();
        assert!(!results.is_empty());
    }
    
    #[test]
    #[ignore] // Stress test - run manually
    fn test_concurrent_layer_access_stress() {
        // This specifically tests that different threads can access different layers
        // of the same node concurrently without deadlocks or data races
        let config = HnswConfig::default();
        let index = Arc::new(HnswIndex::new(128, config));
        
        // Insert a bunch of vectors to create multi-layer nodes
        for i in 0..10_000 {
            let mut vector = vec![0.0; 128];
            vector[0] = (i as f32).sin();
            vector[1] = (i as f32).cos();
             vector[2] = i as f32;
            index.insert(i, vector).unwrap();
        }
        
        // Now hammer the index with concurrent searches
        // This will cause concurrent access to layers of various nodes
        let num_threads = 32;
        let searches_per_thread = 200;
        
        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let index = Arc::clone(&index);
                thread::spawn(move || {
                    for i in 0..searches_per_thread {
                        let mut query = vec![0.0; 128];
                        query[0] = ((thread_id + i) as f32).sin();
                        query[1] = ((thread_id + i) as f32).cos();
                        query[2] = (thread_id * searches_per_thread + i) as f32;
                        index.search(&query, 20).expect("Search failed");
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
