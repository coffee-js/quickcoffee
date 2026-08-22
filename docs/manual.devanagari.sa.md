# QuickCoffee मार्गदर्शिका (देवनागरी, संस्कृत)

`QuickCoffee` Rust-निर्मितः bytecode-यन्त्रः अस्ति, JavaScript-runtime न। स्रोतः पठ्यते, संकल्यते, परीक्ष्यते, ततः चालयते। prototype-chain, `this`, `eval`, अन्तःस्थ-JavaScript च न सन्ति।

triple-quote heredoc newline रक्षति: `"""…"""` `#{expression}` interpolates, `'''…'''` literal भवति; indentation न छिद्यते, unclosed delimiter lexical-error भवति।

`#` एक-पङ्क्ति-comment आरभते। `### … ###` अनन्तर्निहित block-comment अस्ति; layout तथा parse पूर्वं त्यज्यते, अयुक्त-closure lexical-error भवति।

नामानि Unicode XID नियमं अनुसरन्ति: प्रथमं XID start अथवा `_`, पश्चात् XID continue अथवा `_`। संयोजक-चिह्नानि नाम निरन्तरयन्ति; engine Unicode-normalization न करोति।

CoffeeScript-नामानि type न परिवर्तयन्ति: `yes`/`on` = `true`, `no`/`off` = `false`, `is`/`isnt` strict `==`/`!=` स्तः।

strict अथवा numeric-comparison श्रृङ्खला भवति: `1 < middle() < 3` मध्ये middle एकवारं मूल्यते, पूर्वं false चेत् परं न मूल्यते।

`qcoffee -e "print(range(1, 4))"` प्रयुञ्जीत। `qcoffee --check FILE` स्रोतं compile-verify करोति, न चालयति; `--fuel N` निर्देश-संख्यां सीमयति। `print`, `len`, `type`, `range`, `str`, `keys`, `values`, `join`, `split`, `assert` मानक-सहायकाः; `range(a, b)` मध्ये `b` न गृह्यते।

`qcoffee -` मानक-input तः स्रोतः पठति; `qcoffee --dump-bytecode -` तमेव स्रोतं चालनं विना विच्छिनत्ति।
`qcoffee --stats` instruction-संख्या तथा अवशिष्ट-fuel standard error मध्ये लिखति, stdout अपरिवर्तितं स्थापयति; `--check` अथवा `--dump-bytecode` सह न योज्यम्।

`--` पश्चात् argumentाः साधारण-string-array `argv` रूपेण दीयन्ते: `qcoffee program.qc -- first second` मध्ये `len(argv)` `2` भवति। host-process अथवा environment-object न प्रकाश्यते।

कार्यं `(x) -> expression` अथवा bare-name `left, right -> left + right` इति लिख्यते; lexical-environment गृह्णाति। default, rest, pattern तु parentheses अपेक्षन्ते। अन्तिम-सामान्य-parameter default सहितः भवितुं शक्नोति, यथा `(head, separator = '-') -> expression`; argument अभावे अथवा `nil` दत्ते default-expression कार्यस्य अन्तरे मूल्यते, अतः पूर्व-parameter तथा captured-environment पश्यति। आवश्यक-parameter default-parameter पूर्वं स्थापनीयः। अन्तिमः rest-parameter `(head, tail...) -> expression` शेषान् argumentान् array मध्ये बध्नाति। `qdocco FILE -o FILE.html` साहित्य-दस्तावेजं जनयति; `qtest FILE...` सफलं भवति यदा सर्वेषां अन्तिम-मूल्यं `true` भवति।

`return expression` केवलं कार्यस्य अन्तरे मान्यः, तत् कार्यं शीघ्रं समाप्तं करोति; केवलः `return` `nil` फलति। अन्तःस्थ-कार्यं न अतिक्रामति। सक्रिय-loop शुद्धीकरोति तथा अन्तःतः बहिः `finally` चलयति; `finally` मध्ये return पूर्वफलम् परिवर्तयति। सशर्त-return `if condition then return value` इति लिख्यते।

parameter strict-pattern अपि भवति: `([left, right], {factor}) -> (left + right) * factor`। प्रत्येक argument pattern अनुरूपः भवेत्; default केवलं name-parameter, rest केवलं अन्तिम-name भवति।

map-literal मध्ये `{name}` इति `{name: name}` संक्षिप्त-रूपम्; string-key स्पष्ट-मूल्यम् अपेक्षते।

assignment-pattern array तथा map मध्ये nested भवितुं शक्नोति: `[first, {point: [x, y]}] = [1, {point: [20, 22]}]`। array प्रत्येकस्तरे exact-length अपेक्षते, map निर्दिष्ट-key अपेक्षते; VM सर्व-pattern परीक्ष्य पश्चात् एव binding परिवर्तयति।

array-item अथवा call-argument पश्चात् `...` array-विस्तारं करोति: `[1, values..., 4]` तत्त्वानि योजयति, `fn(values...)` पृथक् argument ददाति। विस्तृत-वस्तु array भवेत्।

nil-सुरक्षित suffix CoffeeScript-रीत्या `record?.name`, `values?[index]`, `fn?(args)` इति। receiver `nil` चेत् फलम् `nil` भवति, index अथवा argument न मूल्यते; non-nil receiver सामान्य-strict नियमम् अनुसरति।

`qtest --fuel N FILE...` प्रत्येक-document पृथक् instruction-budget ददाति; एकस्य सीमित-loop अन्यस्य budget न क्षिणोति।
`qtest --stats` प्रत्येकस्य documentस्य instruction-संख्या तथा अवशिष्ट-fuel standard error मध्ये लिखति, `ok`-निर्गमं न परिवर्तयति।

क्रमः `for item in items then expression` इति लिख्यते; binding strict-pattern अपि भवति, यथा `for [left, right] in pairs then left + right`, तथा प्रत्येक-item-स्य सर्व-binding पूर्ण-match पश्चात् एव परिवर्तते। body-मूल्यानि नूतन-array मध्ये संगृह्णाति, `when`-अस्वीकृतानि न संगृह्णाति, `break` संगृहीत-prefix ददाति। `by step`, यथा `for item in [1..9] by 3 then expression`, एकवारं-मूल्यितं positive finite integer पदं ददाति। map-क्रमे `by` नास्ति; `break` तथा `continue` अन्तःस्थितं क्रमं नियच्छतः; while, until, loop nil फलन्ति।

स एव collector CoffeeScript-postfix-comprehension अपि स्वीकरोति: `value * 2 for value in items`, अथवा `[value * 2 for value in items]`। brackets केवलं comprehension-सीमा, अतिरिक्त nested-array न; `by`, `when`, map, pattern, `break`, `continue` prefix-रूपस्य नियमैः चलन्ति।

पूर्णाङ्क-range `[1..3]` अन्तं गृह्णाति, अतः `[1, 2, 3]` भवति; `[1...3]` अन्तं न गृह्णाति, अतः `[1, 2]` भवति। सीमा finite integer भवेत्।

postfix `value?` केवलं non-nil परीक्षते: `nil?` false, `false?` तथा `0?` true; unbound-name-error न गोपयति, `left ? right` fallback अपि न।

`name ?= value` केवलं name unbound अथवा nil चेत् value मूल्ययित्वा बध्नाति; non-nil चेत् right side न चलति। केवलं name, member/index/destructuring न; साधारण unbound-name-read error एव।

नाम्नि strict numeric update अपि अस्ति: `next = ++counter` नूतनं मूल्यं ददाति, `previous = counter--` decrement पूर्वं पुरातनं मूल्यं ददाति। केवलं साधारण-name मान्यः।

CoffeeScript-arithmetic मध्ये floor-division `a // b` तथा modulo `a %% b` अपि स्तः; `-7 // 5` `-2`, `-7 %% 5` `3` भवति, सामान्य `%` तु dividend-संकेतं रक्षति।

बिट्-क्रियाः कठोरैः signed 32-bit अङ्कैः भवन्ति: `&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`; स्थानान्तरण-सङ्ख्या 0 तः 31 पर्यन्तं, संयुक्तरूपाणि केवलं नाम्नि।

पङ्क्तेः अन्ते स्पष्टः operatorः चेत् अभिव्यक्तिः अग्रिम-पङ्क्तौ निरन्तरं भवति; निरन्तर-पङ्क्तेः indentation केवलं विन्यासः, layout न परिवर्तयति।

सामान्य-उद्धृत-पाठः पङ्क्त्यन्तरं गन्तुं शक्नोति; नूतन-पङ्क्तिः एकं space भवति, अन्त्यः backslash तु तां निवारयति।

`(1 + 2 * 3) == 7` इव शुद्धं literal-अङ्कगणितं compilation-काले परीक्षित-constant रूपेण सङ्कुच्यते।

array-slice `items[start..end]` अन्तं गृह्णाति, `items[start...end]` अन्तं न गृह्णाति। सीमौ वामतः दक्षिणं एकवारं मूल्येते, finite integer तथा array-सीमा अन्तर्गतौ भवेताम्। negative-index अन्तात् गणयति, `-1` अन्तिमः; array एव छेद्यः, implicit truncation न। receiver nil चेत् `items?[start..end]` nil फलति, सीमौ न मूल्येते।

`left ? right` nil-विशेष-fallback अस्ति: `left` nil भवति चेत् एव `right` मूल्यते। `false`, zero, रिक्त-string, रिक्त-array च रक्षिताः भवन्ति।

`value in array` QuickCoffee-साम्येन array-सदस्यं परीक्षते, `value not in array` तस्य निषेधः। `key of map` map-स्वकीय-string-key परीक्षते, `key not of map` तस्य निषेधः; map मध्ये prototype-key न सन्ति।

`until condition then body` प्रतिलोम-loop अस्ति: यावत् Boolean-condition सत्यम् न भवति तावत् पुनरावर्तते; `break`, `continue`, indentation, fuel नियमाः `while` इव भवन्ति।

वाक्य-स्थाने `n = n + 1 while n < 3` postfix-loop अस्ति, prefix while इव सम्पूर्ण-assignment पुनरावर्तयति; `until` अपि तथा। strict-destructuring body भवति, सामान्य-subexpression अन्तरे न।

`loop body` अनन्तः `while true`-रूपः अस्ति; `break` तं समाप्तं करोति, fuel-सीमा तिष्ठति।

`for`-iterable तथा `then` मध्ये `when condition` स्थापनेन filter भवति: `for n in [1..5] when n > 2 then print(n)` अस्वीकृत-मूल्येषु body न चलति।

host-त्रुटिः संरचिता: `error.kind()` `ErrorKind::Parse`/`Verify`/`Runtime` ददाति, `error.message()` प्रदर्शन-पाठं न विश्लेष्य विवरणं ददाति, `error.position()` कदाचित् एकतः गणितं स्रोत-पङ्क्तिं ददाति।

पुनःचालनाय `Engine::compile_program` एकवारं compile तथा verify कृत्वा साझा `Program` निर्माति, `Context::run_program` तं चालयति; handle-स्य प्रतिलिपिः bytecode न प्रतिलिपयति, पुनः verification अपि न करोति।

`Context::last_execution()` अन्तिम-सफलतायाः वा runtime-विफलतायाः `ExecutionStats` ददाति; `instructions` तथा `fuel_remaining` तत्र स्तः, compilation अथवा verification-दोषः पूर्वलेखं न परिवर्तयति।

> 注：此文件按“天成文”近似“天城文（Devanagari）”的解释提供；若所指为其他语言或文字，可替换为经审订译本。

बहु-पङ्क्ति array तथा map मध्ये comma त्यक्तुं शक्यते; call-argument तथा सामान्य parenthesis मध्ये स्पष्ट-विभागः आवश्यकः।

एकाकी assignment (`record =`) अनन्तरं indentation द्वारा map लिखितुं शक्यते; nested `key: value` prototype-विहीनं भवति, सामान्य continuation न विपर्यस्यते।

एकस्यां logical-line मध्ये call-parenthesis विना अपि शक्यते: `implicit_answer = implicit_add 20, 22`; comparison अथवा layout-boundary मध्ये explicit parenthesis प्रयोजनीया।
