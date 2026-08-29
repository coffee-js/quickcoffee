import { decide } from './policy'

risk_band = host_risk(request.customer_id)
decision = decide(request, risk_band)
host_audit(decision.code)

evaluation_count = 0
evaluation_count++
export result = map_set(decision, 'evaluation', evaluation_count)
