# QuickCoffee मार्गदर्शिका



मानचित्र-विस्तारः पश्चात् लिखिता कुञ्जी पूर्वलिखितां जयति।

मानचित्र-विन्यासे `...metadata` अनुक्तानि कुञ्जीनि गृह्णाति।

ऋण-सूचकाङ्केन क्रमस्य अन्तिमं पदं लभ्यते।

स्रोतः पठ्यते, सत्यापित-bytecode मध्ये संकल्यते, fuel-सीमया चालयते।

`qcoffee -` मानक-input तः QuickCoffee-program पठति।

`qcoffee --quit` एकं Context निर्माय निःशब्दं निर्गच्छति; स्रोत-अन्यexecution-विकल्पैः सह न योज्यम्।

`qcoffee --stats` instruction-संख्या, अवशिष्ट-fuel, hot-path, managed-value-allocation, lexical-environment-allocation च standard error मध्ये लिखति, stdout अपरिवर्तितं स्थापयति; एकमेव source ग्राह्यः, विरोधि execution-mode तु usage-दोषं जनयति।

अन्तःस्थ-module नामयुक्त import/export प्रयुङ्क्ते; `Engine::compile_module` तथा `Context::run_module` host-`ModuleLoader` द्वारा एव source गृह्णीतः, module-global गोप्यं fuel च सर्व-graph मध्ये संयुक्तम्।

`qcoffee --check FILE` स्रोतं verify करोति, न चालयति।

`qcoffee --interactive` (वा `-i`) एकं Context पङ्क्ति-क्रमेण धारयति; `:help` दर्शयति, `:quit` निर्गच्छति।

`qcoffee --interactive --stats` केवलं कार्यितायै वा runtime-दोषयुक्तायै non-empty पङ्क्त्यै instruction तथा fuel लेखं लिखति; parse अथवा verify-दोषे नूतनं लेखं न लिखति।

'a☕中'[1] '☕' अस्ति, 'a☕中'[1..2] '☕中' अस्ति; string-index Unicode-scalar-अनुसारी अस्ति।

`for character, index in 'a☕中' then index` Unicode-scalar-अङ्कान् `[0, 1, 2]` ददाति; string-iteration मध्ये शून्य-वर्जित signed by-क्रमः अस्ति।

`do (name, other) -> ...` तत्क्षणं आह्वयति, बहिः समाननाम-मूल्यानि ददाति; `do -> ...` निरवयवम् अस्ति।

`[head, tail...] = [1, 2, 3]` tail-नाम्नि `[2, 3]` बध्नाति; array-pattern rest अन्तिमः भवति।

`qtest --fuel N` प्रत्येक executable-document पृथक् instruction-budget ददाति।

`qtest --stats` प्रत्येकस्य documentस्य instruction-संख्या तथा अवशिष्ट-fuel standard error मध्ये लिखति, ok-निर्गमं न परिवर्तयति।

`qtest --json` प्रत्येकस्य लेखस्य स्थिरं JSON फलम् एकस्मिन् पङ्क्तौ लिखति; `--stats` stderr मध्ये एव।

`qtest --tap` TAP 13 तथा नियत-सङ्ख्याङ्कितानि फलानि लिखति; `--json` च `--tap` च परस्परं निषिद्धे।

`qtest --filter TEXT` मार्ग-साम्येन परीक्षां चिनोति; `qtest --list` चयनित-पत्राणि केवलं गणयति, न चालयति।

`qcoffee --json` एकस्मिन् प्रयोगे JSON-मूल्यं वा संरचितं दोषं एकया पङ्क्त्या ददाति, CI-होष्ट्रयोः उपयोगाय।

host-error `ErrorKind::Parse`, Verify, Runtime तथा प्रदर्शनात् स्वतन्त्रं विवरणं ददाति; `error.position()` कदाचित् एकतः गणितां स्रोत-पङ्क्तिं ददाति।

`Engine::compile_program` एकवारं verify करोति; `Context::run_program` पुनःचालने अपरिवर्तनीय-सत्यापित-bytecode पुनरुपयुङ्क्ते।

`Program::fingerprint` होस्ट-सञ्चयाय नियतं u64 बीजं ददाति, निष्पादनं न परिवर्तयति।

`qcoffee --fingerprint FILE` सत्यापित-bytecode-कुञ्जीं षोडश लघु-षोडशाधारीय-अङ्कैः दर्शयति, लेखं न चालयति।

`qbench --json` प्रत्येक-सुरक्षित-भारस्य एकं काल-मापन-फलम् लिखति; `--iterations` नमूना-सङ्ख्यां नियच्छति।

प्रत्येक qbench-फलस्य profile_* क्षेत्राणि एकस्मात् अकालित-निष्पादनात् hot-path तथा allocation-event लिखन्ति; `--iterations` अथवा `--repeat` गुणनं न भवति।

`qbench --compare-qjs PATH` उभयोः startup, compilation, precompiled hot execution, end-to-end CLI समयं च पृथक् ददाति। औपचारिक-प्रतिवेदने `--repeat` 11 भवेत्; प्रत्येक-भागे median तथा *_mad_ns स्तः।

बीजाङ्काः Rust-debug-रूपं विना स्पष्ट-नियत-bytecode-संकेतेन निर्मीयन्ते, अतः साधन-रूपपरिवर्तनं सञ्चय-कुञ्जीं न परिवर्तयति।

`qdocco --markdown` टिप्पणीन्, सीमितं QuickCoffee-कोडं, अन्तिम-मूल्यं च पठनीय Markdown-फलके लिखति।

अन्तःस्थापकः चालनयोर्मध्ये `Context::set_fuel` आह्वयितुं शक्नोति; `Context::fuel` वर्तमान-सीमां दर्शयति, वैश्विक-मूल्यानि न नाशयति; with_global तथा with_native क्रमिक-संयोजनाय स्तः।

`cargo run --example embed` लघुं Rust-आश्रयं संयोजयति, वैश्विकं स्थापयति, native-callback योजयति, QuickCoffee च चालयति।

Host `Value::kind()` द्वारा प्रकारं विभजति, `Value::is_nil()` द्वारा nil परीक्षते, आन्तरिक-container न पश्यति।

Cargo-वस्तु-विवरणानि अन्तःस्थापकान् repository, docs.rs-API, README, licence च प्रति नयन्ति।

`Context::last_execution()` instruction-संख्या तथा अवशिष्ट-fuel दर्शयति, VM-frame न प्रकाशयति।

-- पश्चात् argumentाः साधारण-string-array argv रूपेण दीयन्ते।

JavaScript नास्ति: सार्वजनिक prototype-chain, global/free this, eval, अन्तःस्थ-JavaScript च न सन्ति। Indented class, construction, सीमित-receiver, new, private extends-chain, statically-resolved super तथा सुरक्षिततया निर्गच्छत् receiver-bound => अधुना सन्ति।

    class BoundCounter
      constructor: (@value) ->
      callback: ->
        =>
          @value = @value + 1
          @value

    bound_callback = new BoundCounter(40).callback()
    bound_callback()

`#` line-comment अस्ति; `### … ###` non-nesting block-comment layout तथा parse पूर्वं त्यज्यते।

Unicode XID-नामानि संयोजक-चिह्नानि गृह्णन्ति, अतः स्थित इत्यादि नाम executable अस्ति।

`yes/on` true, `no/off` false; `is/isnt` strict-साम्यम् स्तः।

! strict-Bool not-पर्यायः अस्ति; != strict-असाम्यमेव तिष्ठति।

chained-comparison मध्ये मध्य-मूल्य एकवारं, पूर्व-false चेत् short-circuit भवति।

सामान्य-library साधारण-function रूपेण print, len, type, error, range, str, trim, contains, starts_with, ends_with, replace_all, sort, concat, parse_json, encode_json, integer, number, decimal, decimal_div, round_decimal, abs, sum, min, max, keys, values, join, split, assert ददाति; RFC 0139 string-query strict locale-वर्जितः, trim निश्चित-Unicode-White_Space-सारणीं प्रयुङ्क्ते; RFC 0140 sort समान-प्रकार-सीमित-scalar नूतन-stable-array ददाति; RFC 0144 concat द्वौ String अथवा द्वौ Array अपरिवर्तितरूपेण योजयति; RFC 0150 replace_all वामतः दक्षिणं literal-replacement करोति, inserted-text पुनः न परीक्षते, allocation-पूर्वं resource-limit परीक्षते; `error(code, message, data, cause)` sealed Error निर्माति, catch Error गृह्णाति, resource-error न गृह्णाति। Decimal m-प्रत्ययं गृह्णाति; अनन्त-दशमलव-विभागः स्पष्ट-scale-rounding अपेक्षते।

    trimmed_text = trim('\u{3000}coffee ☕\u{3000}')
    contains(trimmed_text, '☕') and starts_with(trimmed_text, 'coffee') and ends_with(trimmed_text, '☕')
    sort(['中', 'a', '☕']) == ['a', '☕', '中']
    concat([1, 2], [3]) == [1, 2, 3] and concat('coffee ', '☕') == 'coffee ☕'
    replace_all('coffee coffee', 'coffee', 'bean') == 'bean bean'

कार्यं lexical-environment गृह्णाति; `y = 2` omitting अथवा nil दत्ते कार्यस्य अन्तरे default भवति; अन्तिमः rest-parameter `tail...` इति लिख्यते।

bare-name lambda `left, right -> left + right` भवति; default, rest, pattern तु parentheses गृह्णन्ति।

`return expression` केवलं वर्तमान-कार्यं समाप्तं करोति; केवलः return nil फलति, loop शुद्धीकरोति, finally च चलयति।

parameter strict array/map-pattern गृह्णाति; default तथा rest केवलं name भवतः।

पूर्णाङ्क-range `[1..3]` अन्तं गृह्णाति; `[1...3]` अन्तं न गृह्णाति।

Range अधोमुखोऽपि भवति: `[3..1]` `[3, 2, 1]` ददाति, `[3...1]` `[3, 2]` ददाति।

array-slice `a[start..end]` अन्तं गृह्णाति, `a[start...end]` अन्तं न गृह्णाति; सीमा finite integer array-अन्तर्गतौ, negative अन्तात्, nil-safe slice nil-receiver मध्ये सीमौ न मूल्यते।

nil-विशेष-fallback `left ? right` इति; false तथा zero न परिवर्तेते।

postfix `value?` non-nil एव परीक्षते: `nil?` false, `false?` तथा `0?` true; unbound-name-error न गोप्यते।

`name ?= value` unbound अथवा nil नाम्नि एव लिखति; non-nil right-side त्यजति, member/index/pattern न।

नाम्नि strict prefix/postfix update अपि स्तः: `next = ++counter` नूतनं, `previous = counter--` पूर्व-मूल्यं ददाति; केवलं name मान्यः।

arithmetic मध्ये floor-division // तथा dividend-dependent modulo %% अपि स्तः: `-7 // 5` = -2, `-7 %% 5` = 3।

`value in array` array-सदस्यं परीक्षते; `key of map` map-स्वकीय-string-key परीक्षते।

`value not in array` तथा `key not of map` तयोः strict निषेधौ स्तः, prototype विना।

map-literal मध्ये `{name}` इति `{name: name}` संक्षेपः अस्ति।

assignment-pattern array-map nested भवति; VM सर्वं परीक्ष्य पश्चात् एव binding परिवर्तयति।

array तथा call मध्ये `items...` array-विस्तारः, JavaScript apply विना।

nil-सुरक्षित suffix `a?.name`, `a?[i]`, `f?(args)` केवलम् nil-receiver मध्ये short-circuit करोति।

`until condition then body` पुनः पुनः, यावत् Boolean-condition सत्यम् भवति।

वाक्य-स्थाने postfix `while/until` पूर्ण-assignment अथवा strict-destructuring पुनरावर्तयति, सामान्य-subexpression न।

`loop body` अनन्तः `while true`; break निर्गमं करोति, fuel सीमा तिष्ठति।

for-expression शरीर-मूल्यानि नूतने array मध्ये सञ्चिनोति; when तथा continue त्यजतः, break सञ्चित-पूर्वभागं रक्षति।

for-binding strict-pattern भवति: `for [left, right] in pairs` प्रत्येक-pair पूर्णतया बध्नाति।

array-for `by step` उपयुज्यते; non-zero finite integer step एकवारं मूल्यते, negative क्रमः अन्तिम-पदात् आरभते, map तु न।

array-for शून्यात् गणितं index अपि बध्नाति: `for value, index in items then value + index`।

postfix-comprehension समानं strict-collection वहति: `value * 2 for value in items`, अथवा `[value * 2 for value in items]`।

    base = 40
    add = (x) -> x + base
    shorthand = 'yes'
    [first, {point: [x, y]}] = [0, {point: [20, 22]}]
    scale = ([left, right], {factor}) -> (left + right) * factor
    add(2) == 42 and "फलम् #{add(2)}" == 'फलम् 42' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((mukha, y = 2) -> mukha + y)(40) == 42 and ((mukha, puchcha...) -> mukha + len(puchcha))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
    पदयोग = 0
    for n in [1..9] by 3 then पदयोग = पदयोग + n
    पदयोग == 12
    len(for [left, right] in [[20, 22], [1, 2]] then left + right) == 2
    postfix_doubles = value * 2 for value in [1..3]
    postfix_doubles == [2, 4, 6]
    counter_update = 2
    prefix_update = ++counter_update
    postfix_update = counter_update--
    [prefix_update, postfix_update, counter_update] == [3, 3, 3]
    [-7 // 5, -7 %% 5] == [-2, 3]
    [5 & 3, 5 | 2, 5 ^ 1, ~1, 1 << 3, -8 >> 2, -1 >>> 1] == [1, 7, 4, -2, 8, -2, 2147483647]
    continued = 1 +
      2 * 3
    continued == 7
    message = "hello
      world"
    message == 'hello world'
    escaped = "A\\x42\\u{43}"
    escaped == 'ABC'
    folded = (1 + 2 * 3) == 7
    folded
    values = [
      1
      2
    ]
    values == [1, 2]
    record = {
      first: 20
      second: 22
    }
    record.first + record.second == 42
    indented_record =
      first: 20
      nested:
        second: 22
    indented_record.nested.second == 22
    implicit_add = (left, right) -> left + right
    implicit_answer = implicit_add 20, 22
    implicit_answer == 42
    स्थित = 40
    स्थित + 2 == 42
    3 not in [1, 2] and 'missing' not of {present: 1}
    loop_count = 0
    loop
      loop_count = loop_count + 1
      break if loop_count == 3
    loop_count == 3
    bare_add = left, right -> left + right
    bare_add(20, 22) == 42
    postfix_count = 0
    postfix_count = postfix_count + 1 while postfix_count < 3
    postfix_count == 3
    slice_values = [0..4][1..3]
    len(slice_values) == 3 and slice_values[0] == 1 and [0..4][-3...-1][0] == 2
    nil? == false and false? == true and 0? == true
    default_value ?= 42
    default_value == 42
    ### invalid ` source अत्र उपेक्षितः
    ###
    0.1m + 0.2m == 0.3m and decimal_div(1m, 3m, 2, 'half_even') == 0.33m
    json_payload = parse_json('{"money":12.30,"large":9007199254740993}')
    encode_json(json_payload) == '{"large":9007199254740993,"money":12.3}'
    42 == 42
