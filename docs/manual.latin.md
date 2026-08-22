# Manuale QuickCoffee (Latine)

QuickCoffee est machina bytecodicis Rustiana, non tempus JavaScript. Fons legitur, compilatur, verificatur, deinde currit. Nulla catena prototyporum, `this`, `eval`, nec JavaScript inclusum est.

Heredoc trium signorum lineas servat: `"""…"""` `#{expression}` interpolat, `'''…'''` litteralis manet; indentatio non tollitur, delimiter non clausus error lexicalis est.

`#` commentarium unius lineae incipit. `### … ###` commentarium non-nidificatum est, ante layout et analysin remotum; delimiter non clausus error lexicalis est.

Nomina regulas Unicode XID sequuntur: XID start vel `_` primum, XID continue vel `_` postea. Signa combinantia nomen continuare possunt; machina Unicode non normalizat.

Voces CoffeeScript sine mutatione generum valent: `yes`/`on` sunt `true`, `no`/`off` sunt `false`, atque `is`/`isnt` sunt stricta `==`/`!=`.

Comparationes strictae vel numericae conecti possunt: `1 < middle() < 3` medium semel aestimat et, priore falso, posteriora non aestimat.

Utere `qcoffee -e "print(range(1, 4))"`; `qcoffee --check FILE` fontem compilat atque verificat sine cursu; `--fuel N` numerum instructionum finit. Bibliotheca parva habet `print`, `len`, `type`, `range(a, b)`, `str`, `keys`, `values`, `join`, `split`, et `assert`; finis in `range` exclusus est.

`qcoffee -` fontem ex initio normali legit; `qcoffee --dump-bytecode -` illum fontem sine cursu explicat.

Argumenta post `--` ut series chordarum ordinaria `argv` praebentur: in `qcoffee program.qc -- first second`, `len(argv)` est `2`. Nulla res processus aut ambitus hospitis exponitur.

Functio scribitur `(x) -> expressio` vel nominibus nudis, ut `sinister, dexter -> sinister + dexter`, ambitum lexicalem capit. Default, rest, et pattern parentheses poscunt. Parameter extremus valorem praedefinitum habere potest, ut `(caput, separator = '-') -> expressio`; argumento omisso vel `nil` dato, valor intra functionem aestimatur atque parametros priores ambitumque captum videre potest. Parametri necessari ante praedefinitos sunt. Ultimus rest, ut `(caput, cauda...) -> expressio`, reliqua argumenta in serie ligat. Ad documentum faciendum: `qdocco FILE -o FILE.html`. Ad probationes: `qtest FILE...`; omnis valor ultimus `true` esse debet.

`return expressio` solum intra functionem valet eamque statim finit; nudum `return` dat `nil`. Functionem inclusam non transit. Iterationem activam purgat et `finally` circumstantia ab intimo ad externum peragit; return in `finally` eventum priorem superat. Return condicionale scribitur `if conditio then return valor`.

Parametri etiam patterna stricta habere possunt: `([left, right], {factor}) -> (left + right) * factor`. Quodque argumentum pattern convenire debet; praedefinitio solum nomini et rest solum nomini ultimo datur.

In littera map, `{name}` est forma brevis `{name: name}`; claves chordarum valorem explicitum poscunt.

Patterna assignmentis series et maps includere possunt: `[first, {point: [x, y]}] = [1, {point: [20, 22]}]`. Series longitudinem exactam poscunt, maps claves nominatas; VM totum pattern ante mutationem ligaminum verificat.

`...` post item seriei aut argumentum vocationis seriem expandit: `[1, values..., 4]` elementa coniungit et `fn(values...)` singula argumenta tradit. Res expansa series esse debet.

Suffixa tuta nil more CoffeeScript scribuntur: `record?.name`, `values?[index]`, `fn?(args)`. Si recipiens est `nil`, eventus est `nil` nec index aut argumenta aestimantur; recipiens non-nil regulas strictas ordinarias sequitur.

`qtest --fuel N FILE...` singulis documentis budget instructionum separatum dat; una iteratio finita alterius budget non consumit.

Ordo scribitur `for item in items then expressio`; ligamen pattern strictum esse potest, ut `for [left, right] in pairs then left + right`, et omnes ligamina cuiusque item solum post integram congruentiam mutantur. Valores corporis in seriem novam colligit, valores `when` reiecti non colliguntur, et `break` praefixum collectum reddit. `by step`, ut `for item in [1..9] by 3 then expressio`, gradum semel aestimatum et integrum finitum positivum dat. Maps `by` non accipiunt; `break` et `continue` ordinem intimum regunt; while, until, loop nil dant.

Eadem collectio formam postfixam CoffeeScript habet: `value * 2 for value in items`, vel `[value * 2 for value in items]`. Bracteae solum terminum comprehensionis indicant nec seriem interiorem addunt; `by`, `when`, maps, patterna, `break`, et `continue` regulas formae praefixae servant.

Spatium integrorum `[1..3]` finem includit atque `[1, 2, 3]` facit; `[1...3]` finem excludit atque `[1, 2]` facit. Fines integri finiti esse debent.

Suffixum `value?` tantum non-nil probat: `nil?` false est, `false?` et `0?` true sunt; errorem nominis non ligati non celat neque recessus `left ? right` est.

`name ?= value` value aestimat et ligat tantum si nomen non ligatum aut nil est; valore non-nil dextram omittit. Solum nomen admittitur, non membrum, index, aut destructio; lectio ordinaria nominis non ligati error manet.

Nomina etiam strictum incrementum et decrementum habent: `next = ++counter` novum valorem reddit, `previous = counter--` priorem reddit ante decrementum. Tantum nomina simplicia admittuntur.

Arithmetica CoffeeScript etiam divisionem inferiorem `a // b` et modulum `a %% b` praebet; `-7 // 5` est `-2`, `-7 %% 5` est `3`, dum `%` reliquum signum dividendi servat.

Operationes bitwise numeris strictis signatis 32-bit utuntur: `&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`; numerus translationis a 0 ad 31 tantum admittitur, formae compositae nomen solum accipiunt.

Operator explicitus in fine lineae expressionem in linea sequenti continuat; indentatio continuationis ordinem clausularum non mutat.

Textus inter notas simplices vel duplices lineas transire potest; novum-linea fit spatium unum, backslash finalis autem eam tollit.

Arithmetica pura litteralis, ut `(1 + 2 * 3) == 7`, tempore compilationis in constantes verificatas redigitur.

Sectio seriei `items[start..end]` finem includit, `items[start...end]` excludit; termini sinistro ad dextrum semel aestimantur atque integri finiti intra limites esse debent. Numerus negativus ab extremo numeratur, `-1` ultimum est; sola series secari potest, nec truncatio tacita fit. Recipiente nil, `items?[start..end]` nil dat nec terminos aestimat.

`left ? right` recessus nil-specialis est: `right` tantum aestimatur si `left` est `nil`. `false`, zero, chorda vacua, et series vacua servantur.

`value in array` membrum seriei aequalitate QuickCoffee probat, et `value not in array` contrarium. `key of map` clavem propriam map probat, et `key not of map` contrarium; map claves prototyporum non habet.

`until condition then body` forma inversa ordinis est: repetit donec conditio Boolean vera sit; regulae `break`, `continue`, indentationis et fuel eae sunt ac `while`.

In loco sententiae, `n = n + 1 while n < 3` est ordo postfixus aequalis while praefixo et totam assignationem repetit; `until` similiter. Destructio stricta corpus esse potest, non autem subexpressio ordinaria.

`loop body` est forma infinita `while true`; `break` eam finit, atque limite fuel manet.

`when condition` inter iterabile `for` et `then` positum iterationem filtrat: `for n in [1..5] when n > 2 then print(n)` corpus pro valoribus reiectis non currit.

Hospes errorem structum accipit: `error.kind()` dat `ErrorKind::Parse`, `Verify`, aut `Runtime`; `error.message()` detail sine analysi textus ostensi dat, et `error.position()` lineam fontis a uno numeratam interdum dat.

Ad iterandum programmatum compilatum, `Engine::compile_program` dat `Program` commune et `Context::run_program` illud currit; clavis eius sine copia bytecodicis clonatur.

`Context::last_execution()` reddit `ExecutionStats` publicas de ultimo cursu prospero vel errore temporis, cum `instructions` et `fuel_remaining`; errores compilationis vel verificationis memoriam priorem servant.

In seriebus et mapis per plures lineas, commata omitti possunt; argumenta functionum et parenteses ordinariae separationem apertam servant.

Post assignationem solam (`record =`) mapa per indentationem scribi potest; claves interiores sine prototypo fiunt, nec continuatio ordinaria confunditur.

In una linea logica, functio sine parenthesibus vocari potest: `implicit_answer = implicit_add 20, 22`; apud comparationes vel limites ordinis parenthesibus uti licet.
