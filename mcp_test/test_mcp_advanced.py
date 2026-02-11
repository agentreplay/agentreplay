import requests
import sseclient
import json
import threading
import time
import uuid
import sys

# Install dependencies if missing
try:
    import sseclient
except ImportError:
    print("Please run: pip install sseclient-py requests")
    sys.exit(1)

SERVER_URL = "http://localhost:3000"
SSE_URL = f"{SERVER_URL}/sse"

def run_tests():
    print(f"Connecting to SSE at {SSE_URL}...")
    headers = {'Accept': 'text/event-stream'}
    response = requests.get(SSE_URL, stream=True, headers=headers)
    
    client = sseclient.SSEClient(response)
    
    endpoint_data = {"url": None, "session_id": None}
    responses = {} # Map id -> response
    ready_event = threading.Event()
    
    def listen():
        print("Listening for events...")
        for event in client.events():
            # print(f"Received event: {event.event}")
            if event.event == 'endpoint':
                endpoint_url = event.data
                print(f"Endpoint URL received: {endpoint_url}")
                if endpoint_url.startswith("/"):
                    endpoint_url = f"{SERVER_URL}{endpoint_url}"
                
                endpoint_data["url"] = endpoint_url
                if 'sessionId=' in endpoint_url:
                    session_id = endpoint_url.split('sessionId=')[1]
                    endpoint_data["session_id"] = session_id
                    print(f"Session ID: {session_id}")
                ready_event.set()
                
            elif event.event == 'message':
                # Parse JSON-RPC message
                try:
                    msg = json.loads(event.data)
                    # print(f"Received message: {msg}")
                    if 'id' in msg:
                        responses[msg['id']] = msg
                except Exception as e:
                    print(f"Failed to parse message event: {e}")

    thread = threading.Thread(target=listen)
    thread.daemon = True
    thread.start()
            
    print("Waiting for endpoint...")
    if not ready_event.wait(timeout=5):
        print("Timed out waiting for endpoint URL.")
        return
        
    endpoint_url = endpoint_data["url"]

    # Now we can send POST requests to the endpoint
    print(f"\nTesting Tools at {endpoint_url}...")
    
    def rpc_call_async(method, params=None):
        req_id = str(uuid.uuid4())
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or {},
            "id": req_id
        }
        
        try:
            res = requests.post(endpoint_url, json=payload)
            if res.status_code not in [200, 202]:
                print(f"Error Status: {res.status_code}, Body: {res.text}")
                return None
            
            # Wait for response in SSE stream
            start_time = time.time()
            while time.time() - start_time < 5:
                if req_id in responses:
                    return responses.pop(req_id)
                time.sleep(0.1)
            
            print(f"Timeout waiting for response to {method} (id={req_id})")
            return None
        except Exception as e:
            print(f"Request failed: {e}")
            return None

    # 1. List Tools
    print("\n--- Listing Tools ---")
    list_res = rpc_call_async("tools/list")
    if list_res and 'result' in list_res:
        tools = list_res['result'].get('tools', [])
        tool_names = [t['name'] for t in tools]
        print(f"Tools found: {tool_names}")
        
        if "process_order" in tool_names: print("✅ process_order found")
        else: print("❌ process_order missing")
        
        if "generate_error" in tool_names: print("✅ generate_error found")
        else: print("❌ generate_error missing")
    else:
        print(f"failed to list tools: {list_res}")

    # 2. Call process_order
    print("\n--- Calling process_order ---")
    order_payload = {
        "name": "process_order",
        "arguments": {
            "order": {
                "id": "TEST-1",
                "items": [{"name": "Item A", "quantity": 2, "price": 50}],
                "shipping": {"address": "123 Test St", "expedited": False}
            }
        }
    }
    call_res = rpc_call_async("tools/call", order_payload)
    if call_res and 'result' in call_res:
        content = call_res['result']['content'][0]['text']
        print(f"Order Result: {content}")
        if "processed" in content and "100" in content:
            print("✅ process_order Output Correct")
        else:
            print("❌ process_order Output Incorrect")
    else:
        print(f"process_order failed: {call_res}")

    # 3. Call generate_error
    print("\n--- Calling generate_error ---")
    error_payload = {
        "name": "generate_error",
        "arguments": {"message": "Test Failure"}
    }
    err_res = rpc_call_async("tools/call", error_payload)
    if err_res and 'error' in err_res:
        msg = err_res['error']['message']
        print(f"Caught expected error: {msg}")
        if "Simulated Failure: Test Failure" in msg:
            print("✅ Error Handling Correct")
        else:
            print("❌ Error Message Mismatch")
    else:
        print(f"Unexpected success: {err_res}")

    # 4. Resources
    print("\n--- Reading config://app ---")
    res_payload = {"uri": "config://app"}
    read_res = rpc_call_async("resources/read", res_payload)
    if read_res and 'result' in read_res:
        content = read_res['result']['contents'][0]['text']
        print(f"Resource Content: {content}")
        if "test" in content:
            print("✅ Resource Read Correct")
        else:
            print("❌ Resource Content Mismatch")
    else:
        print(f"Resource read failed: {read_res}")

    # 5. Prompts
    print("\n--- Getting code_review prompt ---")
    prom_payload = {
        "name": "code_review",
        "arguments": {"code": "print('hello')"}
    }
    prom_res = rpc_call_async("prompts/get", prom_payload)
    if prom_res and 'result' in prom_res:
        msg = prom_res['result']['messages'][0]['content']['text']
        print(f"Prompt Message: {msg[:50]}...")
        if "Please review this code" in msg:
            print("✅ Prompt Get Correct")
        else:
            print("❌ Prompt Message Mismatch")
    else:
        print(f"Prompt get failed: {prom_res}")

    print("\nAll Tests Completed.")

if __name__ == "__main__":
    run_tests()
