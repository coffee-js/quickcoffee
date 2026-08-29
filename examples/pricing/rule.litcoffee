# Exact pricing and eligibility rule

This executable Literate QuickCoffee module is the canonical implementation used by the
CLI demo, Rust embedding example, integration tests, and benchmarks. Inline identifiers
such as `Decimal`, `Integer`, `quote_order`, and `pricing.ineligible` remain ordinary
Markdown inline code on GitHub; executable code uses the four-space indentation required
by Literate CoffeeScript.

The rule deliberately keeps transport, storage, clocks, and other system capabilities in
the host. It accepts one immutable order Map and returns a new deterministic quote Map.

这份可执行文档也是 CLI、Rust 宿主、集成测试和 benchmark 共用的唯一规则源码。规则只处理
不可变订单 Map 与确定性金额计算；传输、存储、时钟和其他系统能力继续由宿主显式提供。

    invalid_order = (field, expected) ->
      throw error('pricing.invalid_order', 'invalid pricing order', {
        field: field
        expected: expected
      })

    require_field = (order, field) ->
      invalid_order(field, 'required') unless field of order

    validate_order = (order) ->
      invalid_order('order', 'map') unless type(order) == 'map'

      require_field(order, 'subtotal')
      require_field(order, 'item_count')
      require_field(order, 'customer_tier')
      require_field(order, 'country')

      invalid_order('subtotal', 'decimal') unless type(order.subtotal) == 'decimal'
      invalid_order('item_count', 'integer') unless type(order.item_count) == 'integer'
      invalid_order('customer_tier', 'string') unless type(order.customer_tier) == 'string'
      invalid_order('country', 'string') unless type(order.country) == 'string'

      invalid_order('customer_tier', 'member or standard') unless order.customer_tier in ['member', 'standard']
      invalid_order('country', 'CN or US') unless order.country in ['CN', 'US']

      if order.item_count < 1n or order.subtotal < 10m
        throw error('pricing.ineligible', 'order does not meet pricing eligibility', {
          minimum_item_count: 1n
          minimum_subtotal: 10m
        })

      true

    quote_order = (order) ->
      validate_order(order)

      discount_rate = if order.customer_tier == 'member' then 0.10m else 0m
      tax_rate = if order.country == 'CN' then 0.13m else 0.07m
      discount = round_decimal(order.subtotal * discount_rate, 2, 'half_even')
      net = round_decimal(order.subtotal - discount, 2, 'half_even')
      tax = round_decimal(net * tax_rate, 2, 'half_even')

      {
        discount: discount
        net: net
        subtotal: order.subtotal
        tax: tax
        total: round_decimal(net + tax, 2, 'half_even')
      }

    export { quote_order }
