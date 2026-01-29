#!/usr/bin/env python3

# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Basic Dataset Workflow Example

This example demonstrates how to:
1. Create a dataset
2. Add test cases
3. Query dataset contents
4. Delete items

Usage:
    python examples/evals/basic_dataset.py

Requirements:
    - Agentreplay server running at localhost:8000
    - pip install httpx  (or use agentreplay SDK)
"""

import json
import sys
from datetime import datetime
from typing import Any

# Use httpx for HTTP requests (no external dependencies required if using stdlib)
try:
    import httpx
    USE_HTTPX = True
except ImportError:
    import urllib.request
    import urllib.error
    USE_HTTPX = False


class AgentreplayEvalsClient:
    """Simple client for Agentreplay Evals API."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url.rstrip("/")
        if USE_HTTPX:
            self.client = httpx.Client(timeout=30.0)
    
    def _request(self, method: str, path: str, data: dict = None) -> dict:
        """Make HTTP request."""
        url = f"{self.base_url}{path}"
        
        if USE_HTTPX:
            if method == "GET":
                response = self.client.get(url)
            elif method == "POST":
                response = self.client.post(url, json=data)
            elif method == "DELETE":
                response = self.client.delete(url)
            else:
                raise ValueError(f"Unknown method: {method}")
            
            if response.status_code >= 400:
                raise Exception(f"HTTP {response.status_code}: {response.text}")
            
            return response.json() if response.text else {}
        else:
            # Fallback to urllib
            req = urllib.request.Request(url, method=method)
            req.add_header("Content-Type", "application/json")
            
            if data:
                req = urllib.request.Request(
                    url,
                    data=json.dumps(data).encode("utf-8"),
                    method=method,
                    headers={"Content-Type": "application/json"}
                )
            
            try:
                with urllib.request.urlopen(req, timeout=30) as response:
                    return json.loads(response.read().decode("utf-8")) if response.read() else {}
            except urllib.error.HTTPError as e:
                raise Exception(f"HTTP {e.code}: {e.read().decode('utf-8')}")
    
    # Dataset CRUD operations
    
    def create_dataset(
        self,
        name: str,
        description: str = "",
        metadata: dict = None
    ) -> dict:
        """Create a new evaluation dataset."""
        return self._request("POST", "/api/v1/evals/datasets", {
            "name": name,
            "description": description,
            "metadata": metadata or {}
        })
    
    def list_datasets(self) -> list:
        """List all datasets."""
        return self._request("GET", "/api/v1/evals/datasets")
    
    def get_dataset(self, dataset_id: str) -> dict:
        """Get dataset by ID."""
        return self._request("GET", f"/api/v1/evals/datasets/{dataset_id}")
    
    def delete_dataset(self, dataset_id: str) -> None:
        """Delete a dataset."""
        self._request("DELETE", f"/api/v1/evals/datasets/{dataset_id}")
    
    # Dataset items
    
    def add_items(
        self,
        dataset_id: str,
        items: list[dict]
    ) -> dict:
        """Add test cases to a dataset."""
        return self._request("POST", f"/api/v1/evals/datasets/{dataset_id}/items", {
            "items": items
        })
    
    def get_items(self, dataset_id: str) -> list:
        """Get all items in a dataset."""
        return self._request("GET", f"/api/v1/evals/datasets/{dataset_id}/items")


def main():
    print("=" * 60)
    print("Agentreplay Evals - Basic Dataset Workflow")
    print("=" * 60)
    
    client = AgentreplayEvalsClient()
    
    # Step 1: Create a dataset
    print("\n📦 Creating dataset...")
    try:
        dataset = client.create_dataset(
            name=f"QA Test Dataset {datetime.now().strftime('%Y%m%d_%H%M%S')}",
            description="Test dataset for question-answering evaluation",
            metadata={
                "version": "1.0",
                "created_by": "basic_dataset.py",
                "domain": "general_qa"
            }
        )
        dataset_id = dataset["id"]
        print(f"✅ Created dataset: {dataset['name']}")
        print(f"   ID: {dataset_id}")
    except Exception as e:
        print(f"❌ Failed to create dataset: {e}")
        print("\nMake sure Agentreplay server is running at localhost:8000")
        sys.exit(1)
    
    # Step 2: Add test cases
    print("\n📝 Adding test cases...")
    test_cases = [
        {
            "input": "What is the capital of France?",
            "expected_output": "Paris",
            "metadata": {
                "category": "geography",
                "difficulty": "easy"
            }
        },
        {
            "input": "Explain the theory of relativity in simple terms",
            "expected_output": "Einstein's theory of relativity explains that space and time are interconnected, and that the speed of light is constant for all observers.",
            "metadata": {
                "category": "science",
                "difficulty": "medium"
            }
        },
        {
            "input": "What are the key differences between Python and JavaScript?",
            "expected_output": "Python is a general-purpose language with clean syntax, used for backend, data science, and scripting. JavaScript is primarily for web development, runs in browsers, and uses event-driven programming.",
            "metadata": {
                "category": "programming",
                "difficulty": "medium"
            }
        },
        {
            "input": "How does photosynthesis work?",
            "expected_output": "Photosynthesis is the process plants use to convert sunlight, water, and carbon dioxide into glucose and oxygen.",
            "metadata": {
                "category": "biology",
                "difficulty": "easy"
            }
        },
        {
            "input": "Explain quantum computing to a 10-year-old",
            "expected_output": "Regular computers use bits that are either 0 or 1. Quantum computers use special bits called qubits that can be 0, 1, or both at the same time! This lets them solve some puzzles much faster.",
            "metadata": {
                "category": "technology",
                "difficulty": "hard"
            }
        }
    ]
    
    try:
        result = client.add_items(dataset_id, test_cases)
        print(f"✅ Added {len(test_cases)} test cases")
    except Exception as e:
        print(f"❌ Failed to add items: {e}")
    
    # Step 3: List all datasets
    print("\n📋 Listing datasets...")
    try:
        datasets = client.list_datasets()
        print(f"Found {len(datasets)} dataset(s):")
        for ds in datasets[:5]:  # Show first 5
            print(f"  - {ds['name']} (ID: {ds['id']})")
    except Exception as e:
        print(f"❌ Failed to list datasets: {e}")
    
    # Step 4: Get dataset details
    print("\n🔍 Getting dataset details...")
    try:
        details = client.get_dataset(dataset_id)
        print(f"Dataset: {details['name']}")
        print(f"Description: {details['description']}")
        print(f"Item count: {details.get('item_count', 'N/A')}")
    except Exception as e:
        print(f"❌ Failed to get dataset: {e}")
    
    # Step 5: Get items
    print("\n📄 Getting test cases...")
    try:
        items = client.get_items(dataset_id)
        print(f"Retrieved {len(items)} items:")
        for item in items[:3]:  # Show first 3
            input_preview = item['input'][:50] + "..." if len(item['input']) > 50 else item['input']
            print(f"  - [{item.get('metadata', {}).get('category', 'unknown')}] {input_preview}")
    except Exception as e:
        print(f"❌ Failed to get items: {e}")
    
    print("\n" + "=" * 60)
    print("✨ Dataset workflow complete!")
    print(f"   Dataset ID: {dataset_id}")
    print("=" * 60)
    
    return dataset_id


if __name__ == "__main__":
    main()
