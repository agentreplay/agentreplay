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

//! Privacy tag processor for <private>..</private> redaction.

use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct PrivacyMetadata {
    pub redacted_regions: Vec<RedactedRegion>,
    pub malformed_tags: Vec<MalformedTag>,
    pub nesting_depth_max: usize,
}

#[derive(Debug, Clone)]
pub struct RedactedRegion {
    pub start: usize,
    pub end: usize,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct MalformedTag {
    pub position: usize,
    pub kind: MalformedKind,
}

#[derive(Debug, Clone)]
pub enum MalformedKind {
    UnclosedTag,
    UnmatchedClose,
    NestedTooDeep,
}

/// Privacy processor configuration.
#[derive(Debug, Clone)]
pub struct PrivacyProcessorConfig {
    pub respect_code_blocks: bool,
    pub respect_cdata: bool,
    pub max_nesting_depth: usize,
}

impl Default for PrivacyProcessorConfig {
    fn default() -> Self {
        Self {
            respect_code_blocks: true,
            respect_cdata: true,
            max_nesting_depth: 16,
        }
    }
}

/// Streaming privacy processor with nested tag handling.
pub struct PrivacyTagProcessor {
    config: PrivacyProcessorConfig,
}

impl PrivacyTagProcessor {
    const OPEN_TAG: &'static str = "<private>";
    const CLOSE_TAG: &'static str = "</private>";
    const REDACTION: &'static str = "[REDACTED]";

    pub fn new() -> Self {
        Self {
            config: PrivacyProcessorConfig::default(),
        }
    }

    pub fn with_config(config: PrivacyProcessorConfig) -> Self {
        Self { config }
    }

    pub fn process<'a>(&self, text: &'a str) -> (Cow<'a, str>, PrivacyMetadata) {
        if !text.contains("<private") {
            return (Cow::Borrowed(text), PrivacyMetadata::clean());
        }

        let mut output = String::with_capacity(text.len());
        let mut metadata = PrivacyMetadata::new();
        let mut tag_stack: Vec<(usize, usize)> = Vec::new();
        let mut in_code_block = false;
        let mut in_cdata = false;

        let mut i = 0;
        while i < text.len() {
            let slice = &text[i..];

            if self.config.respect_code_blocks && slice.starts_with("```") {
                in_code_block = !in_code_block;
                if tag_stack.is_empty() {
                    output.push_str("```");
                }
                i += 3;
                continue;
            }

            if self.config.respect_cdata && slice.starts_with("<![CDATA[") {
                in_cdata = true;
                if tag_stack.is_empty() {
                    output.push_str("<![CDATA[");
                }
                i += "<![CDATA[".len();
                continue;
            }

            if self.config.respect_cdata && in_cdata && slice.starts_with("]]>") {
                in_cdata = false;
                if tag_stack.is_empty() {
                    output.push_str("]]>");
                }
                i += 3;
                continue;
            }

            if (self.config.respect_code_blocks && in_code_block) || (self.config.respect_cdata && in_cdata) {
                let next = slice.find('<').map(|p| i + p).unwrap_or(text.len());
                if tag_stack.is_empty() {
                    output.push_str(&text[i..next]);
                }
                i = next;
                continue;
            }

            if slice.starts_with(Self::OPEN_TAG) {
                if tag_stack.len() >= self.config.max_nesting_depth {
                    metadata.malformed_tags.push(MalformedTag {
                        position: i,
                        kind: MalformedKind::NestedTooDeep,
                    });
                    i += Self::OPEN_TAG.len();
                    continue;
                }

                tag_stack.push((i, output.len()));
                i += Self::OPEN_TAG.len();
                continue;
            }

            if slice.starts_with(Self::CLOSE_TAG) {
                if let Some((start_input, start_output)) = tag_stack.pop() {
                    output.truncate(start_output);
                    if tag_stack.is_empty() {
                        output.push_str(Self::REDACTION);
                    }

                    metadata.redacted_regions.push(RedactedRegion {
                        start: start_input,
                        end: i + Self::CLOSE_TAG.len(),
                        depth: tag_stack.len() + 1,
                    });
                    metadata.nesting_depth_max = metadata.nesting_depth_max.max(tag_stack.len() + 1);
                } else {
                    metadata.malformed_tags.push(MalformedTag {
                        position: i,
                        kind: MalformedKind::UnmatchedClose,
                    });
                    output.push_str(Self::CLOSE_TAG);
                }

                i += Self::CLOSE_TAG.len();
                continue;
            }

            if tag_stack.is_empty() {
                let next_tag = slice.find('<').map(|p| i + p).unwrap_or(text.len());
                output.push_str(&text[i..next_tag]);
                i = next_tag;
            } else {
                let next_angle = slice.find('<').map(|p| i + p).unwrap_or(text.len());
                if next_angle == text.len() {
                    i = next_angle;
                } else if text[next_angle..].starts_with(Self::OPEN_TAG)
                    || text[next_angle..].starts_with(Self::CLOSE_TAG)
                {
                    i = next_angle;
                } else {
                    i = next_angle + 1;
                }
            }
        }

        for (start_input, start_output) in tag_stack.into_iter().rev() {
            output.truncate(start_output);
            output.push_str(Self::REDACTION);
            metadata.malformed_tags.push(MalformedTag {
                position: start_input,
                kind: MalformedKind::UnclosedTag,
            });
            metadata.redacted_regions.push(RedactedRegion {
                start: start_input,
                end: text.len(),
                depth: 1,
            });
        }

        (Cow::Owned(output), metadata)
    }
}

impl Default for PrivacyTagProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyMetadata {
    fn clean() -> Self {
        Self {
            redacted_regions: Vec::new(),
            malformed_tags: Vec::new(),
            nesting_depth_max: 0,
        }
    }

    fn new() -> Self {
        Self::clean()
    }

    pub fn had_redactions(&self) -> bool {
        !self.redacted_regions.is_empty()
    }

    pub fn is_clean(&self) -> bool {
        self.redacted_regions.is_empty() && self.malformed_tags.is_empty()
    }
}
