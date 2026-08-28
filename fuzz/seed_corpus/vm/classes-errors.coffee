class Counter
  constructor: (@value) ->
  increment: -> @value = @value + 1
counter = new Counter(1)
try
  throw error('seed', 'expected', {value: counter.increment()})
catch issue
  [issue.code, issue.data.value, counter.value]
