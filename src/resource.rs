/// Stable reason for a resource-boundary failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    /// The per-run instruction budget was exhausted.
    Fuel,
    /// A bytecode function call would exceed the configured nesting depth.
    CallDepth,
    /// The embedding host cancelled the current execution.
    Cancellation,
    /// A JSON input exceeded the configured UTF-8 byte boundary.
    JsonInputBytes,
    /// A JSON encoding exceeded the configured UTF-8 output byte boundary.
    JsonOutputBytes,
    /// A decoded or encoded JSON string exceeded its configured UTF-8 byte boundary.
    JsonStringBytes,
    /// A JSON array or object exceeded its configured item boundary.
    JsonContainerItems,
    /// One JSON operation exceeded its configured total value boundary.
    JsonValueCount,
    /// A JSON array or object exceeded its configured nesting boundary.
    JsonNestingDepth,
}

/// Deterministic data-size boundaries applied by an execution [`crate::Context`].
///
/// The defaults preserve RFC 0138's original fixed JSON guards. A policy is
/// copied into a context with [`crate::Context::with_resource_limits`] or
/// [`crate::Context::set_resource_limits`]; lowering a boundary to zero is
/// valid and rejects every non-empty operation covered by that boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    max_json_input_bytes: usize,
    max_json_output_bytes: usize,
    max_json_string_bytes: usize,
    max_json_container_items: usize,
    max_json_values: usize,
    max_json_nesting_depth: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_json_input_bytes: 1_000_000,
            max_json_output_bytes: 1_000_000,
            max_json_string_bytes: 1_000_000,
            max_json_container_items: 100_000,
            max_json_values: 100_000,
            max_json_nesting_depth: 128,
        }
    }
}

impl ResourceLimits {
    /// Returns the maximum UTF-8 bytes accepted by one `parse_json` call.
    pub fn max_json_input_bytes(&self) -> usize {
        self.max_json_input_bytes
    }

    /// Returns a policy with the maximum JSON input byte count replaced.
    pub fn with_max_json_input_bytes(mut self, limit: usize) -> Self {
        self.max_json_input_bytes = limit;
        self
    }

    /// Returns the maximum UTF-8 bytes emitted by one `encode_json` call.
    pub fn max_json_output_bytes(&self) -> usize {
        self.max_json_output_bytes
    }

    /// Returns a policy with the maximum JSON output byte count replaced.
    pub fn with_max_json_output_bytes(mut self, limit: usize) -> Self {
        self.max_json_output_bytes = limit;
        self
    }

    /// Returns the maximum UTF-8 bytes in one decoded or encoded JSON string.
    pub fn max_json_string_bytes(&self) -> usize {
        self.max_json_string_bytes
    }

    /// Returns a policy with the maximum JSON string byte count replaced.
    pub fn with_max_json_string_bytes(mut self, limit: usize) -> Self {
        self.max_json_string_bytes = limit;
        self
    }

    /// Returns the maximum items accepted in one JSON array or object.
    pub fn max_json_container_items(&self) -> usize {
        self.max_json_container_items
    }

    /// Returns a policy with the maximum JSON container item count replaced.
    pub fn with_max_json_container_items(mut self, limit: usize) -> Self {
        self.max_json_container_items = limit;
        self
    }

    /// Returns the maximum values visited by one JSON parse or encode operation.
    pub fn max_json_values(&self) -> usize {
        self.max_json_values
    }

    /// Returns a policy with the maximum JSON value count replaced.
    pub fn with_max_json_values(mut self, limit: usize) -> Self {
        self.max_json_values = limit;
        self
    }

    /// Returns the maximum nested JSON array/object depth.
    pub fn max_json_nesting_depth(&self) -> usize {
        self.max_json_nesting_depth
    }

    /// Returns a policy with the maximum JSON nesting depth replaced.
    pub fn with_max_json_nesting_depth(mut self, limit: usize) -> Self {
        self.max_json_nesting_depth = limit;
        self
    }
}
