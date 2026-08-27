/// Stable reason for a resource-boundary failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
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
    /// An exact Integer exceeded its configured magnitude bit boundary.
    IntegerBits,
    /// A Decimal coefficient exceeded its configured magnitude bit boundary.
    DecimalCoefficientBits,
    /// A Decimal exceeded its configured normalized base-10 scale boundary.
    DecimalScale,
    /// One collection operation exceeded its configured input item boundary.
    CollectionOperationItems,
    /// One text operation exceeded its configured UTF-8 input byte boundary.
    TextOperationBytes,
    /// A general QuickCoffee string exceeded its configured UTF-8 byte boundary.
    StringBytes,
    /// A general QuickCoffee array exceeded its configured item boundary.
    ArrayItems,
    /// A general QuickCoffee map exceeded its configured entry boundary.
    MapEntries,
    /// A Context would retain more logical managed objects than permitted.
    RetainedManagedObjects,
    /// A Context would retain more logical managed payload bytes than permitted.
    RetainedManagedBytes,
    // Append new categories so existing resource-stop discriminants and the
    // monolithic VM dispatch slow paths remain stable.
    /// One raw QuickCoffee source exceeded its configured UTF-8 byte boundary.
    SourceBytes = 20,
    /// One Program or Module exceeded its recursive bytecode instruction boundary.
    BytecodeInstructions = 21,
    /// One static module graph exceeded its unique canonical-module boundary.
    ModuleGraphModules = 22,
    /// One static module graph exceeded its cumulative raw source byte boundary.
    ModuleGraphSourceBytes = 23,
    /// One execution exceeded its cumulative logical managed-object allocation boundary.
    TransientManagedObjects = 24,
    /// One execution exceeded its cumulative logical managed payload-byte allocation boundary.
    TransientManagedBytes = 25,
}

#[cfg(test)]
mod tests {
    use super::ResourceLimit;

    #[test]
    fn legacy_resource_discriminants_stay_stable() {
        assert_eq!(ResourceLimit::Fuel as u8, 0);
        assert_eq!(ResourceLimit::CallDepth as u8, 1);
        assert_eq!(ResourceLimit::Cancellation as u8, 2);
        assert_eq!(ResourceLimit::RetainedManagedBytes as u8, 18);
        assert_eq!(ResourceLimit::SourceBytes as u8, 20);
        assert_eq!(ResourceLimit::BytecodeInstructions as u8, 21);
        assert_eq!(ResourceLimit::ModuleGraphModules as u8, 22);
        assert_eq!(ResourceLimit::ModuleGraphSourceBytes as u8, 23);
        assert_eq!(ResourceLimit::TransientManagedObjects as u8, 24);
        assert_eq!(ResourceLimit::TransientManagedBytes as u8, 25);
    }
}

/// Deterministic source, bytecode, and static module-graph boundaries.
///
/// These limits apply before execution and remain separate from per-Context
/// [`ResourceLimits`]. Raw source is counted before `.litcoffee`
/// preprocessing, bytecode instructions include recursively nested function
/// and pattern-default chunks, and graph totals charge each canonical module
/// once. Hosts can replace every default explicitly through the builder-style
/// methods below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileLimits {
    max_source_bytes: usize,
    max_bytecode_instructions: usize,
    max_module_graph_modules: usize,
    max_module_graph_source_bytes: usize,
}

impl Default for CompileLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1_000_000,
            max_bytecode_instructions: 1_000_000,
            max_module_graph_modules: 1_024,
            max_module_graph_source_bytes: 16_000_000,
        }
    }
}

impl CompileLimits {
    /// Returns the maximum raw UTF-8 bytes accepted for one source.
    pub fn max_source_bytes(&self) -> usize {
        self.max_source_bytes
    }

    /// Returns a policy with the per-source UTF-8 byte boundary replaced.
    pub fn with_max_source_bytes(mut self, limit: usize) -> Self {
        self.max_source_bytes = limit;
        self
    }

    /// Returns the maximum recursively reachable bytecode instructions in one artifact.
    pub fn max_bytecode_instructions(&self) -> usize {
        self.max_bytecode_instructions
    }

    /// Returns a policy with the recursive bytecode instruction boundary replaced.
    pub fn with_max_bytecode_instructions(mut self, limit: usize) -> Self {
        self.max_bytecode_instructions = limit;
        self
    }

    /// Returns the maximum unique canonical modules in one static graph.
    pub fn max_module_graph_modules(&self) -> usize {
        self.max_module_graph_modules
    }

    /// Returns a policy with the unique canonical-module boundary replaced.
    pub fn with_max_module_graph_modules(mut self, limit: usize) -> Self {
        self.max_module_graph_modules = limit;
        self
    }

    /// Returns the maximum cumulative raw UTF-8 source bytes in one static module graph.
    pub fn max_module_graph_source_bytes(&self) -> usize {
        self.max_module_graph_source_bytes
    }

    /// Returns a policy with the cumulative module-graph source boundary replaced.
    pub fn with_max_module_graph_source_bytes(mut self, limit: usize) -> Self {
        self.max_module_graph_source_bytes = limit;
        self
    }
}

/// Deterministic data-size, retained-state, and per-run allocation boundaries
/// applied by an execution [`crate::Context`].
///
/// The defaults preserve RFC 0135/0137/0138's original fixed numeric and JSON guards. A policy is
/// copied into a context with [`crate::Context::with_resource_limits`] or
/// [`crate::Context::set_resource_limits`]; lowering a boundary to zero is
/// valid; zero-bit numeric limits still permit exact zero, whose magnitude uses no bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    max_json_input_bytes: usize,
    max_json_output_bytes: usize,
    max_json_string_bytes: usize,
    max_json_container_items: usize,
    max_json_values: usize,
    max_json_nesting_depth: usize,
    max_integer_bits: u64,
    max_decimal_coefficient_bits: u64,
    max_decimal_scale: u32,
    max_collection_operation_items: usize,
    max_text_operation_bytes: usize,
    max_string_bytes: usize,
    max_array_items: usize,
    max_map_entries: usize,
    max_retained_managed_objects: u64,
    max_retained_managed_bytes: u64,
    max_transient_managed_objects: u64,
    max_transient_managed_bytes: u64,
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
            max_integer_bits: 1_000_000,
            max_decimal_coefficient_bits: 1_000_000,
            max_decimal_scale: 100_000,
            max_collection_operation_items: 100_000,
            max_text_operation_bytes: 1_000_000,
            max_string_bytes: 1_000_000,
            max_array_items: 100_000,
            max_map_entries: 100_000,
            max_retained_managed_objects: u64::MAX,
            max_retained_managed_bytes: u64::MAX,
            max_transient_managed_objects: u64::MAX,
            max_transient_managed_bytes: u64::MAX,
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

    /// Returns the maximum magnitude bits in one script-observable Integer.
    pub fn max_integer_bits(&self) -> u64 {
        self.max_integer_bits
    }

    /// Returns a policy with the maximum Integer magnitude bit count replaced.
    ///
    /// Values above the implementation ceiling do not raise that independent
    /// compile-time and host-construction safety boundary.
    pub fn with_max_integer_bits(mut self, limit: u64) -> Self {
        self.max_integer_bits = limit;
        self
    }

    /// Returns the maximum magnitude bits in one normalized Decimal coefficient.
    pub fn max_decimal_coefficient_bits(&self) -> u64 {
        self.max_decimal_coefficient_bits
    }

    /// Returns a policy with the maximum Decimal coefficient bit count replaced.
    ///
    /// Values above the implementation ceiling do not raise that independent
    /// compile-time and host-construction safety boundary.
    pub fn with_max_decimal_coefficient_bits(mut self, limit: u64) -> Self {
        self.max_decimal_coefficient_bits = limit;
        self
    }

    /// Returns the maximum normalized fractional base-10 digits in one Decimal.
    pub fn max_decimal_scale(&self) -> u32 {
        self.max_decimal_scale
    }

    /// Returns a policy with the maximum normalized Decimal scale replaced.
    ///
    /// Values above the implementation ceiling do not raise that independent
    /// compile-time and host-construction safety boundary.
    pub fn with_max_decimal_scale(mut self, limit: u32) -> Self {
        self.max_decimal_scale = limit;
        self
    }

    /// Returns the maximum input items processed by one collection operation.
    pub fn max_collection_operation_items(&self) -> usize {
        self.max_collection_operation_items
    }

    /// Returns a policy with the maximum collection-operation item count replaced.
    pub fn with_max_collection_operation_items(mut self, limit: usize) -> Self {
        self.max_collection_operation_items = limit;
        self
    }

    /// Returns the maximum UTF-8 input bytes scanned by one text operation.
    pub fn max_text_operation_bytes(&self) -> usize {
        self.max_text_operation_bytes
    }

    /// Returns a policy with the maximum text-operation input byte count replaced.
    pub fn with_max_text_operation_bytes(mut self, limit: usize) -> Self {
        self.max_text_operation_bytes = limit;
        self
    }

    /// Returns the maximum UTF-8 bytes in one general QuickCoffee string value.
    pub fn max_string_bytes(&self) -> usize {
        self.max_string_bytes
    }

    /// Returns a policy with the maximum general string byte count replaced.
    pub fn with_max_string_bytes(mut self, limit: usize) -> Self {
        self.max_string_bytes = limit;
        self
    }

    /// Returns the maximum items in one general QuickCoffee array value.
    pub fn max_array_items(&self) -> usize {
        self.max_array_items
    }

    /// Returns a policy with the maximum general array item count replaced.
    pub fn with_max_array_items(mut self, limit: usize) -> Self {
        self.max_array_items = limit;
        self
    }

    /// Returns the maximum entries in one general QuickCoffee map value.
    pub fn max_map_entries(&self) -> usize {
        self.max_map_entries
    }

    /// Returns a policy with the maximum general map entry count replaced.
    pub fn with_max_map_entries(mut self, limit: usize) -> Self {
        self.max_map_entries = limit;
        self
    }

    /// Returns the maximum managed objects a Context may retain at execution commit.
    ///
    /// The default `u64::MAX` disables this optional retained-state guard.
    pub fn max_retained_managed_objects(&self) -> u64 {
        self.max_retained_managed_objects
    }

    /// Returns a policy with the retained managed-object commit boundary replaced.
    pub fn with_max_retained_managed_objects(mut self, limit: u64) -> Self {
        self.max_retained_managed_objects = limit;
        self
    }

    /// Returns the maximum logical managed payload bytes a Context may retain at execution commit.
    ///
    /// The default `u64::MAX` disables this optional retained-state guard.
    pub fn max_retained_managed_bytes(&self) -> u64 {
        self.max_retained_managed_bytes
    }

    /// Returns a policy with the retained managed-byte commit boundary replaced.
    pub fn with_max_retained_managed_bytes(mut self, limit: u64) -> Self {
        self.max_retained_managed_bytes = limit;
        self
    }

    /// Returns the maximum logical managed objects allocated during one execution.
    ///
    /// The default `u64::MAX` disables this optional cumulative transient-allocation guard.
    pub fn max_transient_managed_objects(&self) -> u64 {
        self.max_transient_managed_objects
    }

    /// Returns a policy with the per-execution managed-object allocation boundary replaced.
    pub fn with_max_transient_managed_objects(mut self, limit: u64) -> Self {
        self.max_transient_managed_objects = limit;
        self
    }

    /// Returns the maximum logical managed payload bytes allocated during one execution.
    ///
    /// The default `u64::MAX` disables this optional cumulative transient-allocation guard.
    pub fn max_transient_managed_bytes(&self) -> u64 {
        self.max_transient_managed_bytes
    }

    /// Returns a policy with the per-execution managed-byte allocation boundary replaced.
    pub fn with_max_transient_managed_bytes(mut self, limit: u64) -> Self {
        self.max_transient_managed_bytes = limit;
        self
    }
}
