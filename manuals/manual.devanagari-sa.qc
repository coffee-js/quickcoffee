## QuickCoffee मार्गदर्शिका
##
## स्रोतः पठ्यते, सत्यापित-bytecode मध्ये संकल्यते, fuel-सीमया चालयते।
## qcoffee - मानक-input तः QuickCoffee-program पठति।
## qcoffee --check FILE स्रोतं verify करोति, न चालयति।
## qtest --fuel N प्रत्येक executable-document पृथक् instruction-budget ददाति।
## host-error ErrorKind::Parse, Verify, Runtime तथा प्रदर्शनात् स्वतन्त्रं विवरणं ददाति; error.position() कदाचित् एकतः गणितां स्रोत-पङ्क्तिं ददाति।
## Engine::compile_program तथा Context::run_program सत्यापित-bytecode पुनःप्रयोगाय साझीकुरुतः।
## -- पश्चात् argumentाः साधारण-string-array argv रूपेण दीयन्ते।
## JavaScript नास्ति: prototype-chain, this, eval, अन्तःस्थ-JavaScript च न सन्ति।
## # line-comment अस्ति; ### … ### non-nesting block-comment layout तथा parse पूर्वं त्यज्यते।
## Unicode XID-नामानि संयोजक-चिह्नानि गृह्णन्ति, अतः स्थित इत्यादि नाम executable अस्ति।
## yes/on true, no/off false; is/isnt strict-साम्यम् स्तः।
## chained-comparison मध्ये मध्य-मूल्य एकवारं, पूर्व-false चेत् short-circuit भवति।
## सामान्य-library साधारण-function रूपेण print, len, type, range, str, keys, values, join, split, assert ददाति।
## कार्यं lexical-environment गृह्णाति; y = 2 omitting अथवा nil दत्ते कार्यस्य अन्तरे default भवति; अन्तिमः rest-parameter tail... इति लिख्यते।
## bare-name lambda left, right -> left + right भवति; default, rest, pattern तु parentheses गृह्णन्ति।
## return expression केवलं वर्तमान-कार्यं समाप्तं करोति; केवलः return nil फलति, loop शुद्धीकरोति, finally च चलयति।
## parameter strict array/map-pattern गृह्णाति; default तथा rest केवलं name भवतः।
## पूर्णाङ्क-range `[1..3]` अन्तं गृह्णाति; `[1...3]` अन्तं न गृह्णाति।
## array-slice a[start..end] अन्तं गृह्णाति, a[start...end] अन्तं न गृह्णाति; सीमा finite integer array-अन्तर्गतौ, negative अन्तात्, nil-safe slice nil-receiver मध्ये सीमौ न मूल्यते।
## nil-विशेष-fallback left ? right इति; false तथा zero न परिवर्तेते।
## postfix value? non-nil एव परीक्षते: nil? false, false? तथा 0? true; unbound-name-error न गोप्यते।
## name ?= value unbound अथवा nil नाम्नि एव लिखति; non-nil right-side त्यजति, member/index/pattern न।
## नाम्नि strict prefix/postfix update अपि स्तः: next = ++counter नूतनं, previous = counter-- पूर्व-मूल्यं ददाति; केवलं name मान्यः।
## arithmetic मध्ये floor-division // तथा dividend-dependent modulo %% अपि स्तः: -7 // 5 = -2, -7 %% 5 = 3।
## value in array array-सदस्यं परीक्षते; key of map map-स्वकीय-string-key परीक्षते।
## value not in array तथा key not of map तयोः strict निषेधौ स्तः, prototype विना।
## map-literal मध्ये {name} इति {name: name} संक्षेपः अस्ति।
## assignment-pattern array-map nested भवति; VM सर्वं परीक्ष्य पश्चात् एव binding परिवर्तयति।
## array तथा call मध्ये items... array-विस्तारः, JavaScript apply विना।
## nil-सुरक्षित suffix a?.name, a?[i], f?(args) केवलम् nil-receiver मध्ये short-circuit करोति।
## until condition then body पुनः पुनः, यावत् Boolean-condition सत्यम् भवति।
## वाक्य-स्थाने postfix while/until पूर्ण-assignment अथवा strict-destructuring पुनरावर्तयति, सामान्य-subexpression न।
## loop body अनन्तः while true; break निर्गमं करोति, fuel सीमा तिष्ठति।
## for-expression शरीर-मूल्यानि नूतने array मध्ये सञ्चिनोति; when तथा continue त्यजतः, break सञ्चित-पूर्वभागं रक्षति।
## for-binding strict-pattern भवति: for [left, right] in pairs प्रत्येक-pair पूर्णतया बध्नाति।
## array-for by step उपयुज्यते; positive finite integer step एकवारं मूल्यते, map तु न।
## array-for शून्यात् गणितं index अपि बध्नाति: for value, index in items then value + index।
## postfix-comprehension समानं strict-collection वहति: value * 2 for value in items, अथवा [value * 2 for value in items]।
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
42 == 42
