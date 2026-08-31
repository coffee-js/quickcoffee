import { quote_order } from './rule.litcoffee'

invalid_config = (field, expected) ->
  throw error('pricing.invalid_config', 'invalid pricing configuration', {
    field: field
    expected: expected
  })

require_field = (value, field, prefix) ->
  invalid_config("#{prefix}.#{field}", 'required') unless field of value

order_from_config = (config, name) ->
  invalid_config('config', 'map') unless type(config) == 'map'
  require_field(config, name, 'config')

  value = config[name]
  invalid_config(name, 'map') unless type(value) == 'map'
  require_field(value, 'subtotal', name)
  require_field(value, 'item_count', name)
  require_field(value, 'customer_tier', name)
  require_field(value, 'country', name)

  invalid_config("#{name}.subtotal", 'decimal string') unless type(value.subtotal) == 'string'
  invalid_config("#{name}.item_count", 'integer') unless type(value.item_count) == 'integer'
  invalid_config("#{name}.customer_tier", 'string') unless type(value.customer_tier) == 'string'
  invalid_config("#{name}.country", 'string') unless type(value.country) == 'string'

  {
    subtotal: decimal(value.subtotal)
    item_count: integer(value.item_count)
    customer_tier: value.customer_tier
    country: value.country
  }

invalid_config('argv[0]', 'canonical JSON from qcson') unless len(argv) == 1
config = parse_json(argv[0])
invalid_config('config', 'map') unless type(config) == 'map'
invalid_config('schema', 'pricing-orders/v1') unless config.schema == 'pricing-orders/v1'

export quote = quote_order(order_from_config(config, 'accepted'))

export rejection = try
  quote_order(order_from_config(config, 'rejected'))
catch problem
  problem.code
