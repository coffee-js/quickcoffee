import { normalize_task } from './task'

source = if len(argv) > 0 then argv[0] else '{"name":"  Write docs  ","tags":[" ux ","daily"]}'

export result = normalize_task(source)
