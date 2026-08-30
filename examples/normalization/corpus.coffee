export sample_input = '''{
  "transient": "discarded root field",
  "schema": "profile-events/v1",
  "events": [
    {
      "id": "  evt-002  ",
      "name": "　Ada ☕　",
      "tags": ["中", "a", "☕"],
      "amount": 12.30,
      "sequence": 9007199254740993,
      "note": "  priority  ",
      "metadata": {"source": "  portal  ", "channel": "web", "transient": true}
    },
    {
      "id": "evt-001",
      "name": "  林 中  ",
      "tags": ["rust", "coffee", "coffee"],
      "amount": 100.00,
      "sequence": 2,
      "note": "　　",
      "metadata": {"source": " app ", "channel": "mobile"}
    }
  ]
}'''

export sample_output = '{"events":[{"amount":12.3,"id":"evt-002","metadata":{"channel":"web","source":"portal"},"name":"Ada ☕","note":"priority","sequence":9007199254740993,"tags":["a","☕","中"]},{"amount":100.0,"id":"evt-001","metadata":{"channel":"mobile","source":"app"},"name":"林 中","sequence":2,"tags":["coffee","coffee","rust"]}],"schema":"profile-events/v1"}'

export unsupported_input = '{"events":[],"schema":"profile-events/v2"}'
export invalid_event_input = '{"events":[{"amount":1.0,"id":"event","metadata":{"source":"host"},"name":"name","sequence":1,"tags":[1]}],"schema":"profile-events/v1"}'
