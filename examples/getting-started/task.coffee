invalid_input = (field, expected) ->
  throw error('input.invalid', 'invalid task input', {
    field: field
    expected: expected
  })

export normalize_task = (source) ->
  invalid_input('source', 'JSON string') unless type(source) == 'string'
  input = parse_json(source)
  invalid_input('input', 'map') unless type(input) == 'map'
  invalid_input('name', 'string') unless 'name' of input and type(input.name) == 'string'
  invalid_input('tags', 'array') unless 'tags' of input and type(input.tags) == 'array'

  name = trim(input.name)
  invalid_input('name', 'non-empty string') if len(name) == 0

  tags = for tag, index in input.tags
    invalid_input("tags[#{index}]", 'string') unless type(tag) == 'string'
    cleaned = trim(tag)
    invalid_input("tags[#{index}]", 'non-empty string') if len(cleaned) == 0
    cleaned

  {
    name: name
    tags: sort(tags)
  }
