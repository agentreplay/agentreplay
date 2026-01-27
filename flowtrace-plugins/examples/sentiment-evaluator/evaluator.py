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
Sentiment Evaluator - A Flowtrace Evaluator Plugin

Evaluates the sentiment of LLM outputs using keyword-based analysis.
Returns a score from 0 (negative) to 1 (positive).
"""

from flowtrace_plugin import (
    Evaluator,
    TraceContext,
    EvalResult,
    PluginMetadata,
    Host,
    export,
)


# Sentiment word lists
POSITIVE_WORDS = [
    "good", "great", "excellent", "amazing", "wonderful", "fantastic",
    "helpful", "thank", "thanks", "perfect", "love", "best", "awesome",
    "appreciate", "happy", "pleased", "satisfied", "brilliant", "superb",
    "outstanding", "remarkable", "exceptional", "positive", "success",
]

NEGATIVE_WORDS = [
    "bad", "terrible", "awful", "horrible", "wrong", "error", "fail",
    "hate", "worst", "poor", "disappointed", "frustrating", "annoying",
    "useless", "broken", "stupid", "dumb", "pathetic", "disgusting",
    "negative", "problem", "issue", "bug", "crash",
]


class SentimentEvaluator(Evaluator):
    """Evaluates the sentiment of LLM outputs."""

    def __init__(self):
        config = Host.get_config()
        self.positive_threshold = config.get("positive_threshold", 0.6)
        self.negative_threshold = config.get("negative_threshold", 0.4)

    def evaluate(self, trace: TraceContext) -> EvalResult:
        """Evaluate the sentiment of a trace's output."""
        # Get the output text to analyze
        output = trace.output or ""
        
        # Also check LLM span outputs
        for span in trace.spans:
            if span.span_type.value == "llm_call" and span.output:
                output += " " + span.output

        if not output:
            return EvalResult(
                evaluator_id="sentiment-evaluator",
                passed=True,
                confidence=0.5,
                explanation="No output text to analyze",
            )

        # Calculate sentiment score
        score, positive_count, negative_count = self._analyze_sentiment(output)

        # Determine pass/fail
        if score >= self.positive_threshold:
            passed = True
            sentiment = "positive"
        elif score <= self.negative_threshold:
            passed = False
            sentiment = "negative"
        else:
            passed = True
            sentiment = "neutral"

        return EvalResult(
            evaluator_id="sentiment-evaluator",
            passed=passed,
            confidence=abs(score - 0.5) * 2,  # Distance from neutral
            explanation=f"Sentiment: {sentiment} (score: {score:.2f})",
            metrics={
                "sentiment_score": score,
                "positive_words": positive_count,
                "negative_words": negative_count,
                "total_words": len(output.split()),
            },
        )

    def _analyze_sentiment(self, text: str) -> tuple:
        """Analyze sentiment using keyword matching."""
        text_lower = text.lower()
        words = text_lower.split()

        positive_count = sum(
            1 for word in words 
            if any(p in word for p in POSITIVE_WORDS)
        )
        negative_count = sum(
            1 for word in words 
            if any(n in word for n in NEGATIVE_WORDS)
        )

        total = positive_count + negative_count
        if total == 0:
            return 0.5, 0, 0  # Neutral

        # Score from 0 to 1
        score = (positive_count + 0.5 * total) / (2 * total)
        return score, positive_count, negative_count

    def get_metadata(self) -> PluginMetadata:
        return PluginMetadata(
            id="sentiment-evaluator",
            name="Sentiment Evaluator",
            version="0.1.0",
            description="Evaluates the sentiment of LLM outputs",
            author="Flowtrace Team",
            tags=["evaluator", "sentiment", "nlp"],
        )


# Export the plugin
export(SentimentEvaluator())
