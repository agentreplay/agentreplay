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
Privacy utilities for payload redaction and PII scrubbing.

Provides configurable redaction to protect sensitive data before sending
to the Agentreplay backend.

Example:
    >>> from agentreplay import init
    >>> from agentreplay.privacy import configure_privacy
    >>> 
    >>> init()
    >>> 
    >>> configure_privacy(
    ...     redact_patterns=[r"\\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Z|a-z]{2,}\\b"],
    ...     scrub_paths=["input.password", "output.api_key"],
    ...     hash_pii=True,
    ... )
"""

import re
import hashlib
import logging
from typing import Any, Dict, List, Optional, Pattern, Callable, Union
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)

# =============================================================================
# Privacy Configuration
# =============================================================================

@dataclass
class PrivacyConfig:
    """Privacy configuration for payload redaction."""
    
    # Enable/disable privacy features
    enabled: bool = True
    
    # Patterns to redact (compiled regex)
    redact_patterns: List[Pattern] = field(default_factory=list)
    
    # JSON paths to scrub entirely (e.g., "input.password")
    scrub_paths: List[str] = field(default_factory=list)
    
    # Whether to hash PII instead of replacing with [REDACTED]
    hash_pii: bool = False
    
    # Salt for PII hashing
    hash_salt: str = ""
    
    # Custom redaction function
    custom_redactor: Optional[Callable[[str], str]] = None
    
    # Replacement text for redacted content
    redacted_text: str = "[REDACTED]"


# Global privacy config
_privacy_config: PrivacyConfig = PrivacyConfig()


# =============================================================================
# Built-in Patterns
# =============================================================================

# Common PII patterns
PATTERNS = {
    "email": re.compile(
        r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b',
        re.IGNORECASE
    ),
    "credit_card": re.compile(
        r'\b(?:\d{4}[-\s]?){3}\d{4}\b'
    ),
    "ssn": re.compile(
        r'\b\d{3}-\d{2}-\d{4}\b'
    ),
    "phone_us": re.compile(
        r'\b(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b'
    ),
    "phone_intl": re.compile(
        r'\b\+\d{1,3}[-.\s]?\d{1,4}[-.\s]?\d{1,4}[-.\s]?\d{1,9}\b'
    ),
    "api_key": re.compile(
        r'\b(?:sk-|pk_|api_|key_|secret_)[A-Za-z0-9_-]{20,}\b',
        re.IGNORECASE
    ),
    "bearer_token": re.compile(
        r'Bearer\s+[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+',
        re.IGNORECASE
    ),
    "jwt": re.compile(
        r'\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b'
    ),
    "password_field": re.compile(
        r'(?i)(password|passwd|pwd|secret|token|api_key|apikey)["\']?\s*[:=]\s*["\']?[^"\'\s,}]+',
    ),
    "ip_address": re.compile(
        r'\b(?:\d{1,3}\.){3}\d{1,3}\b'
    ),
}


# =============================================================================
# Configuration Functions
# =============================================================================

def configure_privacy(
    *,
    enabled: bool = True,
    redact_patterns: Optional[List[Union[str, Pattern]]] = None,
    scrub_paths: Optional[List[str]] = None,
    hash_pii: bool = False,
    hash_salt: str = "",
    custom_redactor: Optional[Callable[[str], str]] = None,
    redacted_text: str = "[REDACTED]",
    use_builtin_patterns: bool = True,
) -> None:
    """Configure privacy settings for payload redaction.
    
    Args:
        enabled: Enable/disable privacy features
        redact_patterns: Regex patterns (str or compiled) to redact
        scrub_paths: JSON paths to completely remove (e.g., "input.password")
        hash_pii: Hash PII values instead of replacing with [REDACTED]
        hash_salt: Salt for PII hashing
        custom_redactor: Custom function for redaction
        redacted_text: Text to use for redacted content
        use_builtin_patterns: Include built-in patterns for common PII
        
    Example:
        >>> configure_privacy(
        ...     redact_patterns=[r"secret-\\w+"],
        ...     scrub_paths=["input.api_key", "output.credentials"],
        ...     hash_pii=True,
        ... )
    """
    global _privacy_config
    
    patterns: List[Pattern] = []
    
    # Add built-in patterns
    if use_builtin_patterns:
        patterns.extend(PATTERNS.values())
    
    # Add custom patterns
    if redact_patterns:
        for pattern in redact_patterns:
            if isinstance(pattern, str):
                patterns.append(re.compile(pattern))
            else:
                patterns.append(pattern)
    
    _privacy_config = PrivacyConfig(
        enabled=enabled,
        redact_patterns=patterns,
        scrub_paths=scrub_paths or [],
        hash_pii=hash_pii,
        hash_salt=hash_salt,
        custom_redactor=custom_redactor,
        redacted_text=redacted_text,
    )


def get_privacy_config() -> PrivacyConfig:
    """Get current privacy configuration."""
    return _privacy_config


def reset_privacy() -> None:
    """Reset privacy configuration to defaults."""
    global _privacy_config
    _privacy_config = PrivacyConfig()


# =============================================================================
# Redaction Functions
# =============================================================================

def redact_payload(payload: Any) -> Any:
    """Redact sensitive data from a payload.
    
    Args:
        payload: Any JSON-serializable data
        
    Returns:
        Redacted payload
        
    Example:
        >>> data = {"email": "user@example.com", "password": "secret123"}
        >>> redacted = redact_payload(data)
        >>> print(redacted)
        {'email': '[REDACTED]', 'password': '[REDACTED]'}
    """
    if not _privacy_config.enabled:
        return payload
    
    return _redact_value(payload, path="")


def _redact_value(value: Any, path: str = "") -> Any:
    """Recursively redact values in a data structure."""
    # Check if path should be scrubbed entirely
    if path and _should_scrub_path(path):
        return _privacy_config.redacted_text
    
    if isinstance(value, dict):
        return {
            k: _redact_value(v, f"{path}.{k}" if path else k)
            for k, v in value.items()
        }
    elif isinstance(value, list):
        return [
            _redact_value(item, f"{path}[{i}]")
            for i, item in enumerate(value)
        ]
    elif isinstance(value, str):
        return _redact_string(value)
    else:
        return value


def _should_scrub_path(path: str) -> bool:
    """Check if a path should be completely scrubbed."""
    path_lower = path.lower()
    for scrub_path in _privacy_config.scrub_paths:
        if path_lower == scrub_path.lower() or path_lower.endswith(f".{scrub_path.lower()}"):
            return True
    return False


def _redact_string(value: str) -> str:
    """Redact patterns from a string value."""
    if not value:
        return value
    
    result = value
    
    # Apply custom redactor first
    if _privacy_config.custom_redactor:
        result = _privacy_config.custom_redactor(result)
    
    # Apply pattern-based redaction
    for pattern in _privacy_config.redact_patterns:
        if _privacy_config.hash_pii:
            result = pattern.sub(
                lambda m: _hash_value(m.group(0)),
                result
            )
        else:
            result = pattern.sub(_privacy_config.redacted_text, result)
    
    return result


def _hash_value(value: str) -> str:
    """Hash a PII value for consistent but anonymized tracking."""
    salted = f"{_privacy_config.hash_salt}{value}"
    hash_bytes = hashlib.sha256(salted.encode()).digest()
    # Return first 8 chars of hex hash with prefix
    return f"[HASH:{hash_bytes[:4].hex()}]"


# =============================================================================
# Convenience Functions
# =============================================================================

def redact_string(value: str) -> str:
    """Redact patterns from a single string.
    
    Args:
        value: String to redact
        
    Returns:
        Redacted string
    """
    if not _privacy_config.enabled:
        return value
    return _redact_string(value)


def hash_pii(value: str, salt: Optional[str] = None) -> str:
    """Hash a PII value for anonymous tracking.
    
    Creates a consistent hash so you can track unique values
    without storing the actual PII.
    
    Args:
        value: The PII value to hash
        salt: Optional salt (defaults to configured salt)
        
    Returns:
        Hashed value like "[HASH:a1b2c3d4]"
    """
    salt = salt or _privacy_config.hash_salt
    salted = f"{salt}{value}"
    hash_bytes = hashlib.sha256(salted.encode()).digest()
    return f"[HASH:{hash_bytes[:4].hex()}]"


def add_pattern(pattern: Union[str, Pattern], name: Optional[str] = None) -> None:
    """Add a redaction pattern at runtime.
    
    Args:
        pattern: Regex pattern (string or compiled)
        name: Optional name for the pattern
        
    Example:
        >>> add_pattern(r"secret-\\w+", name="custom_secret")
    """
    if isinstance(pattern, str):
        pattern = re.compile(pattern)
    
    _privacy_config.redact_patterns.append(pattern)
    
    if name:
        logger.debug(f"Added privacy pattern: {name}")


def add_scrub_path(path: str) -> None:
    """Add a path to scrub at runtime.
    
    Args:
        path: JSON path to scrub (e.g., "input.credentials")
    """
    _privacy_config.scrub_paths.append(path)
    logger.debug(f"Added scrub path: {path}")


# =============================================================================
# Mask Functions (for display)
# =============================================================================

def mask_email(email: str) -> str:
    """Mask an email address for display.
    
    Args:
        email: Email address
        
    Returns:
        Masked email like "u***@example.com"
    """
    if "@" not in email:
        return email
    
    local, domain = email.rsplit("@", 1)
    if len(local) <= 1:
        masked_local = "*"
    else:
        masked_local = f"{local[0]}{'*' * (len(local) - 1)}"
    
    return f"{masked_local}@{domain}"


def mask_phone(phone: str) -> str:
    """Mask a phone number for display.
    
    Args:
        phone: Phone number
        
    Returns:
        Masked phone like "***-***-1234"
    """
    # Keep only digits
    digits = re.sub(r'\D', '', phone)
    if len(digits) < 4:
        return "*" * len(phone)
    
    return f"***-***-{digits[-4:]}"


def mask_credit_card(cc: str) -> str:
    """Mask a credit card for display.
    
    Args:
        cc: Credit card number
        
    Returns:
        Masked card like "****-****-****-1234"
    """
    digits = re.sub(r'\D', '', cc)
    if len(digits) < 4:
        return "*" * len(cc)
    
    return f"****-****-****-{digits[-4:]}"


# =============================================================================
# Context Manager for Temporary Privacy Settings
# =============================================================================

class privacy_context:
    """Context manager for temporary privacy settings.
    
    Example:
        >>> with privacy_context(redact_patterns=[r"secret-\\w+"]):
        ...     # Additional patterns active only in this block
        ...     result = redact_payload(data)
    """
    
    def __init__(
        self,
        *,
        redact_patterns: Optional[List[Union[str, Pattern]]] = None,
        scrub_paths: Optional[List[str]] = None,
    ):
        self.extra_patterns = redact_patterns or []
        self.extra_paths = scrub_paths or []
        self._original_patterns: List[Pattern] = []
        self._original_paths: List[str] = []
    
    def __enter__(self):
        global _privacy_config
        
        # Save originals
        self._original_patterns = _privacy_config.redact_patterns.copy()
        self._original_paths = _privacy_config.scrub_paths.copy()
        
        # Add extra patterns
        for pattern in self.extra_patterns:
            if isinstance(pattern, str):
                _privacy_config.redact_patterns.append(re.compile(pattern))
            else:
                _privacy_config.redact_patterns.append(pattern)
        
        # Add extra paths
        _privacy_config.scrub_paths.extend(self.extra_paths)
        
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        global _privacy_config
        
        # Restore originals
        _privacy_config.redact_patterns = self._original_patterns
        _privacy_config.scrub_paths = self._original_paths
        
        return False
