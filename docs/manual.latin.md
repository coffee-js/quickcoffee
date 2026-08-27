# QuickCoffee document

## Notes

# Manuale QuickCoffee



Tabulae mappae segmenta expandere possunt; claves posteriores priores superant.

In forma mappae `...metadata` claves omissas immutabiliter capit.

Indices negativi in seriebus et textu Unicode extremum elementum petunt.

Fons legitur, in bytecodicem verificatum compilatur, et cum limite fuel currit.

`qcoffee -` programma QuickCoffee ex initio normali legit.

`qcoffee --quit` unum Context instituit et tacite exit; cum fonte aliisque modis executionis coniungi non potest.

`qcoffee --stats` numeros instructionum, alimenti reliqui, viarum calidarum, valorum curatorum allocatarum et ambituum lexicalium allocatarum ad errorem ordinarium scribit, dum exitus programmatis intactus manet; unus tantum fons admittitur et modi contrarii errorem usus reddunt.

Moduli hospitis import/export nominata habent; `Engine::compile_module` et `Context::run_module` fontem solum per `ModuleLoader` hospitis accipiunt, globalia privata servant, et fuel per graphum communicant.

`Engine::fingerprint_module_graph` per eundem loader totum graphum legit et verificat sine exsecutione, clavem u64 versionatam fontibus, nominibus canonicis, import/export et marginibus sensibilem reddens.

`qcoffee --check FILE` fontem verificat sine cursu.

`qcoffee --interactive` (vel `-i`) unum Context per lineas servat; `:help` docet, `:quit` exit.

`qcoffee --interactive --stats` unam instructionum et alimenti reliqui notam lineae non vacuae exsecutæ vel errorem currendi ferenti scribit; errores analysi vel verificationis nihil scribunt.

'a☕中'[1] est '☕', et 'a☕中'[1..2] est '☕中'; indices stringarum scalas Unicode sequuntur.

`for character, index in 'a☕中' then index` indices Unicode scalarum `[0, 1, 2]` reddit; iteratio stringarum gradus nonnullos signatos by accipit.

`do (name, other) -> ...` statim vocat et valores externos eiusdem nominis tradit; `do -> ...` sine argumentis manet.

`[head, tail...] = [1, 2, 3]` tail ad `[2, 3]` ligat; rest in forma array postremum esse debet.

`qtest --fuel N` cuique documento exsecutabili budget instructionum proprium dat.

`qtest --stats` numeros instructionum et alimenti reliqui cuiusque documenti ad errorem ordinarium scribit, sine mutatione exitus ok.

`qtest --json` unam lineam JSON pro unoquoque documento scribit ad usum CI; `--stats` in stderr manet.

`qtest --tap` versiones TAP 13 et numeros certos scribit; `--json` et `--tap` simul prohibentur.

`qtest --filter TEXT` itinera congruentia eligit; `qtest --list` tantum documenta electa enumerat sine exsecutione.

`qcoffee --json` unam lineam JSON valoris vel erroris structi reddit, aptam CI hospitibusque.

Errores hospitis `ErrorKind::Parse`, Verify, Runtime habent atque detail sine textu ostenso praebent; `error.position()` lineam fontis a uno numeratam interdum dat.

`Engine::compile_program` semel verificat; `Context::run_program` bytecode immutabile verificatum ad iteratum cursum reutitur.

`Program::fingerprint` clavem u64 determinatam praebet ad memoriam hospitis sine mutatione exsecutionis.

`qcoffee --fingerprint FILE` eandem clavem hexadecimali parvis litteris sedecim signorum ostendit, sine documento exsecuto.

`qcoffee --fingerprint --module-root ROOT ENTRY` radicem restrictam aperte concedit et clavem v1 totius graphi eiusdem formae ostendit, nullo modulo exsecuto.

`qbench --json` unam mensurae lineam pro unoquoque onere custodito emittit; `--iterations` numerum exemplorum regit.

Campi profile_* cuiusque lineae qbench ex uno cursu non mensurato veniunt et vias calidas allocationesque sine multiplicatione per `--iterations` aut `--repeat` referunt.

`qbench --compare-qjs PATH` initium, compilationem, cursum calidum praecompilatum et totum tempus CLI utriusque runtime separat. Relationes `--repeat` 11 utantur; cuique parti mediana et *_mad_ns sunt.

Claves codicem bytecode explicite et canonice signant, non formam Rust debug; ideo mutatio instrumenti claves non mutat.

`qdocco --markdown` notas, codicem QuickCoffee clausum, et valorem ultimum in documento Markdown scribit.

Hospes inter cursus `Context::set_fuel` vocare potest; `Context::fuel` budgetum ostendit sine globalibus deletis; `with_global` et `with_native` configurationem concatenatam praebent.

`Runtime::context_builder` contextus separatos creat qui tantum caches finitas Program/Module verificatorum communicant; globalia, exportata aestimata, alimentum, cancellatio, statistica et memoria retenta cuique contextui propria manent.

Native contextualis opt-in per `NativeCallContext` cancellationem explorat, alimentum consumit, allocationes administratas refert, et `HostState` typatum scriptis invisibile accedit sine auctoritate ambiente.

`HostCapabilities` et `CapabilityKey<T>` ansas clock, random, logging, file, network in indice Context proprio ponunt; moduli ansas hereditant, Contextus separati sponte non communicant, et hospes cancellationem, alimentum, allocationes explicite rationem reddit.

`cargo run --example embed` hospitem Rust minimum compilat: globale ponit, callback nativum addit, et QuickCoffee currit.

Hospes `Value::kind()` ad genus discernendum et `Value::is_nil()` ad nil probandum utitur, sine interioribus vasorum.

Notitiae Cargoe hospites ad repositorium, API docs.rs, README et licentiam ducunt.

`Context::last_execution()` numeros instructionum et alimenti reliqui ostendit, sine tabulis VM.

Argumenta post -- ut series chordarum ordinaria argv praebentur.

JavaScript non est: catena prototyporum publica, this globale vel liberum, eval, atque JavaScript inclusum desunt. Classes indentatae, constructio, receptores conclusi, new, catena privata extends, super statice resolutum atque => receptorem ligans et tuto effugiens iam adsunt.



`#` commentarium lineae est; `### … ###` commentarium non-nidificatum ante layout et analysin removetur.

Nomina Unicode XID sequuntur; signa combinantia ea continuant, sine normalizatione.

`yes/on` sunt true, `no/off` false; `is/isnt` aequalitatem strictam servant.

! est negatio Bool stricta sicut not; != inaequalitas stricta manet.

Comparationes conectae medium semel servant atque priore falso breviant.

Bibliotheca communis functiones ordinarias habet: print, len, type, error, range, str, trim, contains, starts_with, ends_with, replace_all, sort, concat, parse_json, encode_json, integer, number, decimal, decimal_div, round_decimal, abs, sum, min, max, keys, values, join, split, assert; RFC 0139 inquisitiones stringarum strictas sine locale definit et trim tabula Unicode White_Space fixa utitur; RFC 0140 sort seriem novam stabilem scalarum finitorum eiusdem generis reddit; RFC 0144 concat duas String aut duas Array immutabiliter coniungit; RFC 0150 replace_all litteras a sinistra ad dextram sine textu inserto iterum quaerendo mutat, et limites ante allocationem probat; `error(code, message, data, cause)` Error clausum facit, catch Error accipit, sed resource-error capi non potest. Decimal m suffixo utitur; divisio non terminans scalam et modum rotundandi apertos poscit.


Functiones ambitum lexicalem capiunt; `y = 2` omissus vel nil intra functionem adhibetur; rest ultimus scribitur `tail...`.

Nomina nuda parentheses omittere possunt: sinister, dexter -> sinister + dexter; default, rest, pattern eas servant.

return expressio functionem praesentem finit; nudum return nil dat, iterata purgat, et finally circumstantia peragit.

Parametri patterna stricta seriei/map habere possunt; default et rest nomina manent.

Spatium integrorum `[1..3]` finem includit; `[1...3]` finem excludit.

Spatia descendere quoque possunt: `[3..1]` `[3, 2, 1]` reddit, `[3...1]` `[3, 2]` reddit.

Sectio seriei `a[start..end]` finem includit, `a[start...end]` excludit; termini integri finiti intra limites sunt, negativi ab extremo numerantur, et sectio nil-tuta terminos nil recipiente omittit.

Recessus nil-specialis `left ? right` scribitur; false et zero servantur.

Suffixum `value?` non-nil tantum probat: `nil?` false est, `false?` et `0?` true sunt, nomen non ligatum errorem manet.

`name ?= value` tantum nomen non ligatum aut nil scribit; non-nil dextram omittit, membrum, index, destructio excluduntur.

Nomina etiam incrementum/decrementum strictum praefixum et postfixum habent: `next = ++counter` novum, `previous = counter--` vetus valorem reddit.

Arithmetica etiam divisionem inferiorem // et modulum dependentem %% habet: `-7 // 5` est -2, `-7 %% 5` est 3.

`value in array` membrum seriei probat; `key of map` clavem propriam map probat.

`value not in array` et `key not of map` easdem probationes strictas negant, sine prototypis.

In littera map, `{name}` pro `{name: name}` breviter scribitur.

Patterna assignmentis series atque maps includere possunt; VM totum ante ligamina mutanda verificat.

In seriebus et vocationibus, `items...` seriem expandit sine JavaScript apply.

Suffixa nil-tuta `a?.name`, `a?[i]`, `f?(args)` tantum recipiens nil breviant.

`until condition then body` repetit donec conditio Boolean vera sit.

In loco sententiae, postfix `while/until` totam assignationem aut destructionem strictam repetit, non subexpressionem.

`loop body` est `while true` infinitum; break exit, fuel autem limitem manet.

Expressio for valores corporis colligit; when et continue omittunt, break praefixum collectum servat.

Ligamen for pattern strictum esse potest: `for [left, right] in pairs` singula par atomice ligat.

Ordo seriei `by step` uti potest; gradus integer finitus positivus semel aestimatur, maps eum excludunt.

Iteratio seriei etiam indicem a zero numeratum ligare potest: `for value, index in items then value + index`.

Comprehensio postfix eandem collectionem strictam servat: `value * 2 for value in items`, vel `[value * 2 for value in items]`.

## Code

````coffee
class BoundCounter
  constructor: (@value) ->
  callback: ->
    =>
      @value = @value + 1
      @value

bound_callback = new BoundCounter(40).callback()
bound_callback()







trimmed_text = trim('\u{3000}coffee ☕\u{3000}')
contains(trimmed_text, '☕') and starts_with(trimmed_text, 'coffee') and ends_with(trimmed_text, '☕')
sort(['中', 'a', '☕']) == ['a', '☕', '中']
concat([1, 2], [3]) == [1, 2, 3] and concat('coffee ', '☕') == 'coffee ☕'
replace_all('coffee coffee', 'coffee', 'bean') == 'bean bean'



























numerus = 7
quadratum = (x) -> x * x
shorthand = 'yes'
[first, {point: [x, y]}] = [0, {point: [20, 22]}]
scale = ([left, right], {factor}) -> (left + right) * factor
quadratum(numerus) == 49 and "numerus=#{quadratum(numerus)}" == 'numerus=49' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((caput, y = 2) -> caput + y)(40) == 42 and ((caput, cauda...) -> caput + len(cauda))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
summa_gradus = 0
for n in [1..9] by 3 then summa_gradus = summa_gradus + n
summa_gradus == 12
len(for [left, right] in [[20, 22], [1, 2]] then left + right) == 2
postfixum_duplum = value * 2 for value in [1..3]
postfixum_duplum == [2, 4, 6]
numerus_mut = 2
praefixum_mut = ++numerus_mut
postfixum_mut = numerus_mut--
[praefixum_mut, postfixum_mut, numerus_mut] == [3, 3, 3]
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
3 not in [1, 2] and 'absens' not of {praesens: 1}
numerus_circuli = 0
loop
  numerus_circuli = numerus_circuli + 1
  break if numerus_circuli == 3
numerus_circuli == 3
additio_nuda = sinister, dexter -> sinister + dexter
additio_nuda(20, 22) == 42
numerus_postfixus = 0
numerus_postfixus = numerus_postfixus + 1 while numerus_postfixus < 3
numerus_postfixus == 3
sectio = [0..4][1..3]
len(sectio) == 3 and sectio[0] == 1 and [0..4][-3...-1][0] == 2
nil? == false and false? == true and 0? == true
valor_defectus ?= 42
valor_defectus == 42
### fons invalidus ` hic ignoratur
###
0.1m + 0.2m == 0.3m and decimal_div(1m, 3m, 2, 'half_even') == 0.33m
json_payload = parse_json('{"money":12.30,"large":9007199254740993}')
encode_json(json_payload) == '{"large":9007199254740993,"money":12.3}'
42 == 42
````

## Final value

`true`
