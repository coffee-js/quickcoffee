import { decide } from './policy'

request = {
  amount: 120m
  country: 'CN'
  customer_id: 'customer-1'
  purpose: 'equipment'
}

approved = decide(request, 'low')
risk_denied = decide(request, 'high')
country_denied = decide(map_set(request, 'country', 'GB'), 'low')
amount_denied = decide(map_set(request, 'amount', 600m), 'medium')
invalid = try
  decide(map_set(request, 'amount', 120), 'low')
catch problem
  [problem.code, problem.data.field, problem.data.expected]

export test = approved.approved and
  approved.code == 'policy.approved' and
  approved.amount == 120m and
  not risk_denied.approved and
  risk_denied.code == 'policy.risk_denied' and
  country_denied.code == 'policy.country_denied' and
  amount_denied.code == 'policy.amount_denied' and
  invalid == ['policy.invalid_request', 'amount', 'decimal']
