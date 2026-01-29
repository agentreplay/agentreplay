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

"""Retry logic with exponential backoff for resilient API calls."""

import time
import logging
from typing import Callable, TypeVar
import httpx
from agentreplay.exceptions import (
    AuthenticationError,
    RateLimitError,
    ServerError,
    ValidationError,
    NotFoundError,
    NetworkError,
)

logger = logging.getLogger(__name__)

T = TypeVar("T")


def retry_with_backoff(
    func: Callable[[], T],
    max_retries: int = 3,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
) -> T:
    """Retry a function with exponential backoff.
    
    Automatically retries on transient failures (network errors, 5xx, 429)
    but fails fast on permanent errors (4xx except 429).
    
    Args:
        func: Function to retry
        max_retries: Maximum number of retry attempts (default: 3)
        base_delay: Initial delay in seconds (default: 1.0)
        max_delay: Maximum delay between retries (default: 60.0)
        
    Returns:
        Result of func()
        
    Raises:
        AuthenticationError: On 401 errors (no retry)
        ValidationError: On 400 errors (no retry)
        NotFoundError: On 404 errors (no retry)
        RateLimitError: On 429 after max retries
        ServerError: On 5xx after max retries
        NetworkError: On network errors after max retries
        
    Example:
        >>> def make_request():
        ...     return client.post("/api/v1/edges", json={...})
        >>> 
        >>> response = retry_with_backoff(make_request, max_retries=5)
    """
    last_exception = None

    for attempt in range(max_retries + 1):
        try:
            return func()

        except httpx.HTTPStatusError as e:
            status_code = e.response.status_code

            # 401 - Authentication error (no retry)
            if status_code == 401:
                logger.error(f"Authentication failed: {e}")
                raise AuthenticationError(str(e))

            # 400 - Validation error (no retry)
            elif status_code == 400:
                logger.error(f"Validation error: {e}")
                raise ValidationError(e.response.text)

            # 404 - Not found (no retry)
            elif status_code == 404:
                logger.error(f"Resource not found: {e}")
                raise NotFoundError(e.request.url.path)

            # 429 - Rate limited (retry with server-specified delay)
            elif status_code == 429:
                retry_after_float = float(e.response.headers.get("Retry-After", base_delay * (2**attempt)))
                retry_after_float = min(retry_after_float, max_delay)
                retry_after = int(retry_after_float)

                if attempt < max_retries:
                    logger.warning(
                        f"Rate limited (attempt {attempt + 1}/{max_retries}). "
                        f"Retrying after {retry_after}s"
                    )
                    time.sleep(retry_after)
                    last_exception = RateLimitError(retry_after)
                    continue
                else:
                    logger.error(f"Rate limited after {max_retries} retries")
                    raise RateLimitError(retry_after)

            # 5xx - Server error (retry with exponential backoff)
            elif status_code >= 500:
                delay = min(base_delay * (2**attempt), max_delay)

                if attempt < max_retries:
                    logger.warning(
                        f"Server error {status_code} (attempt {attempt + 1}/{max_retries}). "
                        f"Retrying after {delay}s"
                    )
                    time.sleep(delay)
                    last_exception = ServerError(status_code, e.response.text)
                    continue
                else:
                    logger.error(f"Server error after {max_retries} retries: {e}")
                    raise ServerError(status_code, e.response.text)

            # Other 4xx errors - no retry
            else:
                logger.error(f"HTTP error {status_code}: {e}")
                raise ValidationError(f"HTTP {status_code}: {e.response.text}")

        except (httpx.RequestError, httpx.ConnectError, httpx.TimeoutException) as e:
            # Network errors - retry with exponential backoff
            delay = min(base_delay * (2**attempt), max_delay)

            if attempt < max_retries:
                logger.warning(
                    f"Network error (attempt {attempt + 1}/{max_retries}): {e}. "
                    f"Retrying after {delay}s"
                )
                time.sleep(delay)
                last_exception = NetworkError(str(e))
                continue
            else:
                logger.error(f"Network error after {max_retries} retries: {e}")
                raise NetworkError(str(e))

    # Should never reach here, but handle gracefully
    if last_exception:
        raise last_exception
    raise NetworkError("Unknown error after retries")
