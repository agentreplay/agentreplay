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
Manual OpenAI Instrumentation Wrapper

Use this if you want more control than auto-patching provides.
"""
import time
from typing import Optional
from openai import OpenAI
from agentreplay import Agentreplay
from agentreplay.genai import GenAISpan


class TracedOpenAI:
    """Wrapper around OpenAI client with Agentreplay integration"""
    
    def __init__(self, api_key: Optional[str] = None, agentreplay: Optional[Agentreplay] = None):
        self.client = OpenAI(api_key=api_key)
        self.agentreplay = agentreplay or Agentreplay()
    
    def chat_completion(self, **kwargs):
        """
        Create chat completion with automatic tracing.
        
        Usage:
            client = TracedOpenAI()
            response = client.chat_completion(
                model="gpt-4o",
                messages=[{"role": "user", "content": "Hello!"}]
            )
        """
        start_time = time.time()
        
        # Extract parameters
        model = kwargs.get('model', 'unknown')
        messages = kwargs.get('messages', [])
        
        # Create span
        span = GenAISpan(
            name="openai.chat.completion",
            system="openai",
            operation_name="chat",
            request_model=model,
            temperature=kwargs.get('temperature'),
            top_p=kwargs.get('top_p'),
            max_tokens=kwargs.get('max_tokens'),
        )
        
        # Add prompts
        for msg in messages:
            span.add_prompt(role=msg.get('role', 'user'), content=msg.get('content', ''))
        
        try:
            # Call OpenAI
            response = self.client.chat.completions.create(**kwargs)
            
            # Extract completion
            if response.choices:
                choice = response.choices[0]
                span.add_completion(
                    role=choice.message.role,
                    content=choice.message.content or "",
                    finish_reason=choice.finish_reason
                )
            
            # Extract usage
            if response.usage:
                span.input_tokens = response.usage.prompt_tokens
                span.output_tokens = response.usage.completion_tokens
                span.total_tokens = response.usage.total_tokens
            
            # Extract metadata
            span.response_model = response.model
            span.response_id = response.id
            
            # Send span
            duration = time.time() - start_time
            self.agentreplay.send_span(span, duration=duration)
            
            return response
            
        except Exception as e:
            # Log error
            duration = time.time() - start_time
            span.add_attribute('error.type', type(e).__name__)
            span.add_attribute('error.message', str(e))
            self.agentreplay.send_span(span, duration=duration)
            raise


# Example usage
if __name__ == "__main__":
    client = TracedOpenAI()
    
    response = client.chat_completion(
        model="gpt-4o-mini",
        messages=[
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is the capital of France?"}
        ],
        temperature=0.7
    )
    
    print(f"Response: {response.choices[0].message.content}")
    print("✅ Trace sent to Agentreplay!")
