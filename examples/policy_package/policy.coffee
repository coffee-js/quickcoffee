import { evaluate_policy } from './core.litcoffee'
import { policy_config } from './config'

export decide = (request, risk_band) ->
  evaluate_policy(request, risk_band, policy_config)
