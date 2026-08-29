# Purchase authorization policy core

This executable Literate QuickCoffee module contains the pure policy used by the
composition module, isolated `qtest` case, Rust host, integration tests, and benchmark.
Inline names such as `purchase-policy/v1`, `policy.invalid_request`, and
`policy.risk_denied` remain ordinary Markdown code on GitHub. Executable code uses the
four-space indentation required by Literate CoffeeScript.

Risk lookup and audit are deliberately absent here. The host supplies those capabilities
at the outer module boundary; the pure core receives only explicit immutable values and
returns a deterministic decision Map. No module gains ambient file, network, clock, or
random authority.

这份可执行 `.litcoffee` 文档是采购授权策略的纯核心。风险查询与审计只由 Rust 宿主
在外层显式注入；核心只处理不可变输入并返回确定性决策，不获得文件、网络、时钟或随机
权限。

From the repository root, run `qtest --module-root examples/policy_package test` to
exercise the callback-free core, or `cargo run --example policy_package` to execute the
same package with typed host state, an allowlisted audit capability, cancellation, fuel,
and logical-memory observation.

`Runtime` and `Context` remain same-thread values; the deployment baseline is one Runtime
and one compiled package per worker, while only `CancellationToken` crosses threads. The
in-process limits below are deterministic defense in depth, not a hostile-code sandbox.
Run genuinely untrusted policies in a separate process with operating-system resource and
termination controls. `HostState` and capability implementations are trusted host code and
must cooperate with fuel, cancellation, and allocation accounting.

`Runtime` 与 `Context` 仍限制在同一线程；当前多 worker 基线是每个 worker 各自持有
Runtime 与已预检 package，只有 `CancellationToken` 可跨线程。进程内资源限制是纵深
防御，不是完整敌对代码沙箱；真正不可信策略必须配合独立进程、操作系统资源和外部终止。

    invalid_request = (field, expected) ->
      throw error('policy.invalid_request', 'invalid purchase request', {
        field: field
        expected: expected
      })

    invalid_config = (field, expected) ->
      throw error('policy.invalid_config', 'invalid purchase policy configuration', {
        field: field
        expected: expected
      })

    require_field = (value, field, fail) ->
      fail(field, 'required') unless field of value

    validate_request = (request) ->
      invalid_request('request', 'map') unless type(request) == 'map'
      for field in ['amount', 'country', 'customer_id', 'purpose']
        require_field(request, field, invalid_request)

      invalid_request('amount', 'decimal') unless type(request.amount) == 'decimal'
      invalid_request('country', 'string') unless type(request.country) == 'string'
      invalid_request('customer_id', 'string') unless type(request.customer_id) == 'string'
      invalid_request('purpose', 'string') unless type(request.purpose) == 'string'
      invalid_request('amount', 'positive decimal') unless request.amount > 0m
      invalid_request('country', 'non-empty string') if len(request.country) == 0
      invalid_request('customer_id', 'non-empty string') if len(request.customer_id) == 0
      invalid_request('purpose', 'non-empty string') if len(request.purpose) == 0
      true

    validate_config = (config) ->
      invalid_config('config', 'map') unless type(config) == 'map'
      for field in ['allowed_countries', 'allowed_purposes', 'max_amount', 'medium_max_amount', 'version']
        require_field(config, field, invalid_config)

      invalid_config('allowed_countries', 'array') unless type(config.allowed_countries) == 'array'
      invalid_config('allowed_purposes', 'array') unless type(config.allowed_purposes) == 'array'
      invalid_config('max_amount', 'decimal') unless type(config.max_amount) == 'decimal'
      invalid_config('medium_max_amount', 'decimal') unless type(config.medium_max_amount) == 'decimal'
      invalid_config('version', 'string') unless type(config.version) == 'string'
      true

    evaluate_policy = (request, risk_band, config) ->
      validate_request(request)
      validate_config(config)
      invalid_request('risk_band', 'low, medium, or high') unless risk_band in ['low', 'medium', 'high']

      approved = true
      code = 'policy.approved'
      limit = if risk_band == 'medium' then config.medium_max_amount else config.max_amount

      if risk_band == 'high'
        approved = false
        code = 'policy.risk_denied'
      else unless request.country in config.allowed_countries
        approved = false
        code = 'policy.country_denied'
      else unless request.purpose in config.allowed_purposes
        approved = false
        code = 'policy.purpose_denied'
      else if request.amount > limit
        approved = false
        code = 'policy.amount_denied'

      {
        amount: request.amount
        approved: approved
        code: code
        country: request.country
        customer_id: request.customer_id
        policy: config.version
        purpose: request.purpose
        risk_band: risk_band
      }

    export { evaluate_policy }
