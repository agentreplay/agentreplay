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

"""Custom exceptions for Agentreplay client."""


class AgentreplayError(Exception):
    """Base exception for all Agentreplay client errors."""
    pass


class AuthenticationError(AgentreplayError):
    """Raised when authentication fails (401 Unauthorized)."""
    pass


class RateLimitError(AgentreplayError):
    """Raised when rate limited (429 Too Many Requests).
    
    Attributes:
        retry_after: Seconds to wait before retrying
    """

    def __init__(self, retry_after: int):
        """Initialize rate limit error.
        
        Args:
            retry_after: Seconds to wait before retrying
        """
        self.retry_after = retry_after
        super().__init__(f"Rate limited. Retry after {retry_after} seconds")


class ServerError(AgentreplayError):
    """Raised on 5xx server errors."""

    def __init__(self, status_code: int, message: str):
        """Initialize server error.
        
        Args:
            status_code: HTTP status code (500-599)
            message: Error message from server
        """
        self.status_code = status_code
        super().__init__(f"Server error ({status_code}): {message}")


class ValidationError(AgentreplayError):
    """Raised on 400 Bad Request / validation errors."""

    def __init__(self, message: str):
        """Initialize validation error.
        
        Args:
            message: Validation error details
        """
        super().__init__(f"Validation error: {message}")


class NotFoundError(AgentreplayError):
    """Raised on 404 Not Found errors."""

    def __init__(self, resource: str):
        """Initialize not found error.
        
        Args:
            resource: Resource that wasn't found
        """
        super().__init__(f"Resource not found: {resource}")


class NetworkError(AgentreplayError):
    """Raised on network/connection errors."""

    def __init__(self, message: str):
        """Initialize network error.
        
        Args:
            message: Network error details
        """
        super().__init__(f"Network error: {message}")
