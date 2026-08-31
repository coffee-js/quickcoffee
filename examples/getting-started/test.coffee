import { normalize_task } from './task'

normalized = normalize_task('{"name":"  Write docs  ","tags":["ux"," daily "]}')

invalid = try
  normalize_task('{"name":"Write docs","tags":[1]}')
catch problem
  [problem.code, problem.data.field]

export test = normalized.name == 'Write docs' and
  normalized.tags == ['daily', 'ux'] and
  invalid == ['input.invalid', 'tags[0]']
