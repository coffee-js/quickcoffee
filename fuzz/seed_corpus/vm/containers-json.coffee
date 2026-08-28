record = parse_json('{"items":[1,2,3],"name":"coffee"}')
values = for value, index in record.items then value + integer(index)
encode_json({name: trim("  #{record.name}  "), values})
