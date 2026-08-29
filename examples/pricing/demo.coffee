import { quote_order } from './rule.litcoffee'

request = {
  subtotal: 120m
  item_count: 3n
  customer_tier: 'member'
  country: 'CN'
}

export quote = quote_order(request)

export rejection = try
  quote_order({
    subtotal: 5m
    item_count: 1n
    customer_tier: 'standard'
    country: 'US'
  })
catch problem
  problem.code
