import { quote_order } from './rule.litcoffee'

quote = quote_order({
  subtotal: 120m
  item_count: 3n
  customer_tier: 'member'
  country: 'CN'
})

rejection = try
  quote_order({
    subtotal: 5m
    item_count: 1n
    customer_tier: 'standard'
    country: 'US'
  })
catch problem
  problem.code

export test = quote.discount == 12m and
  quote.net == 108m and
  quote.tax == 14.04m and
  quote.total == 122.04m and
  rejection == 'pricing.ineligible'
