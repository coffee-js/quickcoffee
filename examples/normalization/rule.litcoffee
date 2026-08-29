# Deterministic profile-event normalization

This executable Literate QuickCoffee module is the only implementation of the
normalization rule. The CLI demo, `qtest` case, Rust host, integration tests, and
benchmark all import it. Markdown identifiers such as `profile-events/v1`,
`normalization.invalid_event`, and `json.parse` remain inline code on GitHub; the
four-space-indented blocks below are executable Literate CoffeeScript.

The host owns file I/O. This rule receives JSON text, validates and reshapes explicit
values, and returns canonical JSON. It deliberately performs no locale-dependent case
mapping or Unicode normalization: `trim` uses QuickCoffee's pinned White_Space table,
and `sort` compares Unicode scalars deterministically.

这份可执行 Literate QuickCoffee 文档是规范化规则的唯一实现。CLI、`qtest`、Rust
宿主、集成测试和 benchmark 都直接导入它。文件 I/O 留给宿主；规则只接收 JSON
文本并返回规范 JSON，不引入 locale、隐式类型转换或环境权限。

From the repository root, run `qcoffee --module-root examples/normalization demo` for
the fixed CLI corpus, `qtest --module-root examples/normalization test` for isolated
acceptance, or `cargo run --example normalization -- examples/normalization/input.v1.json`
to let the Rust host read an external JSON file under explicit host authority. 使用者也可用
这三条命令分别验证固定 CLI corpus、隔离测试和显式 Rust 文件输入。

    invalid_document = (field, expected) ->
      throw error('normalization.invalid_document', 'invalid normalization document', {
        field: field
        expected: expected
      })

    invalid_event = (index, field, expected) ->
      throw error('normalization.invalid_event', 'invalid profile event', {
        event_index: index
        field: field
        expected: expected
      })

    require_document_field = (document, field) ->
      invalid_document(field, 'required') unless field of document

    require_event_field = (event, index, field) ->
      invalid_event(index, field, 'required') unless field of event

    normalize_tags = (tags, event_index) ->
      invalid_event(event_index, 'tags', 'array') unless type(tags) == 'array'

      normalized = for tag, tag_index in tags
        invalid_event(event_index, "tags[#{tag_index}]", 'string') unless type(tag) == 'string'
        value = trim(tag)
        invalid_event(event_index, "tags[#{tag_index}]", 'non-empty string') if len(value) == 0
        value

      sort(normalized)

    normalize_event = (event, index) ->
      invalid_event(index, 'event', 'map') unless type(event) == 'map'

      for field in ['id', 'name', 'tags', 'amount', 'sequence', 'metadata']
        require_event_field(event, index, field)

      invalid_event(index, 'id', 'string') unless type(event.id) == 'string'
      invalid_event(index, 'name', 'string') unless type(event.name) == 'string'
      invalid_event(index, 'amount', 'decimal') unless type(event.amount) == 'decimal'
      invalid_event(index, 'sequence', 'integer') unless type(event.sequence) == 'integer'
      invalid_event(index, 'metadata', 'map') unless type(event.metadata) == 'map'
      invalid_event(index, 'metadata.source', 'required') unless 'source' of event.metadata
      invalid_event(index, 'metadata.source', 'string') unless type(event.metadata.source) == 'string'

      id = trim(event.id)
      name = trim(event.name)
      source = trim(event.metadata.source)
      invalid_event(index, 'id', 'non-empty string') if len(id) == 0
      invalid_event(index, 'name', 'non-empty string') if len(name) == 0
      invalid_event(index, 'metadata.source', 'non-empty string') if len(source) == 0

      metadata = map_delete(event.metadata, 'transient')
      metadata = map_set(metadata, 'source', source)
      normalized = {
        amount: event.amount
        id: id
        metadata: metadata
        name: name
        sequence: event.sequence
        tags: normalize_tags(event.tags, index)
      }

      if 'note' of event
        invalid_event(index, 'note', 'string') unless type(event.note) == 'string'
        note = trim(event.note)
        if len(note) > 0
          normalized = map_set(normalized, 'note', note)

      normalized

    normalize_document = (document) ->
      invalid_document('document', 'map') unless type(document) == 'map'
      require_document_field(document, 'schema')
      require_document_field(document, 'events')
      invalid_document('schema', 'string') unless type(document.schema) == 'string'
      invalid_document('events', 'array') unless type(document.events) == 'array'

      unless document.schema == 'profile-events/v1'
        throw error('normalization.unsupported_schema', 'unsupported profile-event schema', {
          actual: document.schema
          expected: 'profile-events/v1'
        })

      {
        events: normalize_event(event, index) for event, index in document.events
        schema: document.schema
      }

    normalize_json = (source) ->
      invalid_document('source', 'string') unless type(source) == 'string'
      encode_json(normalize_document(parse_json(source)))

    export { normalize_document, normalize_json }
