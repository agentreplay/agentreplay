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
LangChain Auto-Instrumentation via Callback Handler

Add Flowtrace to any LangChain application with one line.
"""
from langchain.callbacks.base import BaseCallbackHandler
from langchain.schema import LLMResult
from typing import Any, Dict, List, Optional
import time

from flowtrace import Flowtrace
from flowtrace.genai import GenAISpan


class FlowtraceCallback(BaseCallbackHandler):
    """
    LangChain callback handler for Flowtrace integration.
    
    Usage:
        from langchain.chat_models import ChatOpenAI
        from flowtrace_callback import FlowtraceCallback
        
        llm = ChatOpenAI(callbacks=[FlowtraceCallback()])
        response = llm.predict("Hello!")
    """
    
    def __init__(self, flowtrace: Optional[Flowtrace] = None):
        self.flowtrace = flowtrace or Flowtrace()
        self.active_spans: Dict[str, tuple[GenAISpan, float]] = {}
    
    def on_llm_start(
        self, serialized: Dict[str, Any], prompts: List[str], **kwargs: Any
    ) -> None:
        """Called when LLM starts running."""
        run_id = kwargs.get('run_id', str(time.time()))
        
        # Extract model info
        model_name = serialized.get('name', 'unknown')
        kwargs_dict = serialized.get('kwargs', {})
        
        # Create span
        span = GenAISpan(
            name=f"langchain.{model_name}",
            system=self._detect_provider(model_name),
            operation_name="completion",
            request_model=kwargs_dict.get('model_name', model_name),
            temperature=kwargs_dict.get('temperature'),
            max_tokens=kwargs_dict.get('max_tokens'),
        )
        
        # Add prompts
        for prompt in prompts:
            span.add_prompt(role="user", content=prompt)
        
        # Store span with timestamp
        self.active_spans[str(run_id)] = (span, time.time())
    
    def on_llm_end(self, response: LLMResult, **kwargs: Any) -> None:
        """Called when LLM ends running."""
        run_id = str(kwargs.get('run_id', ''))
        
        if run_id not in self.active_spans:
            return
        
        span, start_time = self.active_spans.pop(run_id)
        
        # Extract completions
        for generation in response.generations[0]:
            span.add_completion(
                role="assistant",
                content=generation.text,
                finish_reason="stop"
            )
        
        # Extract token usage if available
        if response.llm_output:
            token_usage = response.llm_output.get('token_usage', {})
            span.input_tokens = token_usage.get('prompt_tokens')
            span.output_tokens = token_usage.get('completion_tokens')
            span.total_tokens = token_usage.get('total_tokens')
        
        # Send span
        duration = time.time() - start_time
        self.flowtrace.send_span(span, duration=duration)
    
    def on_llm_error(
        self, error: Exception, **kwargs: Any
    ) -> None:
        """Called when LLM errors."""
        run_id = str(kwargs.get('run_id', ''))
        
        if run_id not in self.active_spans:
            return
        
        span, start_time = self.active_spans.pop(run_id)
        
        # Add error info
        span.add_attribute('error.type', type(error).__name__)
        span.add_attribute('error.message', str(error))
        
        # Send span
        duration = time.time() - start_time
        self.flowtrace.send_span(span, duration=duration)
    
    @staticmethod
    def _detect_provider(model_name: str) -> str:
        """Detect provider from model name."""
        model_lower = model_name.lower()
        if 'openai' in model_lower or 'gpt' in model_lower:
            return "openai"
        elif 'anthropic' in model_lower or 'claude' in model_lower:
            return "anthropic"
        elif 'cohere' in model_lower:
            return "cohere"
        else:
            return "unknown"


# Example usage
if __name__ == "__main__":
    from langchain.chat_models import ChatOpenAI
    from langchain.prompts import ChatPromptTemplate
    from langchain.schema import StrOutputParser
    
    # Create LLM with Flowtrace callback
    llm = ChatOpenAI(
        model="gpt-4o-mini",
        callbacks=[FlowtraceCallback()]
    )
    
    # Create chain
    prompt = ChatPromptTemplate.from_messages([
        ("system", "You are a helpful assistant."),
        ("user", "{input}")
    ])
    
    chain = prompt | llm | StrOutputParser()
    
    # Run chain - automatically traced!
    result = chain.invoke({"input": "What is the capital of France?"})
    
    print(f"Response: {result}")
    print("✅ Trace sent to Flowtrace!")
