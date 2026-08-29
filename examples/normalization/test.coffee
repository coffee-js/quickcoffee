import { normalize_json } from './rule.litcoffee'
import { invalid_event_input, sample_input, sample_output, unsupported_input } from './corpus'

invalid_json = try
  normalize_json('{"schema":')
catch problem
  problem.code

invalid_event = try
  normalize_json(invalid_event_input)
catch problem
  [problem.code, problem.data.field, problem.data.event_index]

unsupported = try
  normalize_json(unsupported_input)
catch problem
  problem.code

export test = normalize_json(sample_input) == sample_output and
  invalid_json == 'json.parse' and
  invalid_event == ['normalization.invalid_event', 'tags[0]', 0] and
  unsupported == 'normalization.unsupported_schema'
