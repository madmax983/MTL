# READ-tax verdict

Both arms answer the *same* item: an MTL program (with the quickref embedded) vs. a semantically identical Python twin (no quickref). `mtl - python` is the read-tax delta; a large negative delta means MTL's token density costs comprehension.

## Headline

- **Comprehension accuracy** — MTL 100.0% vs Python 100.0% (delta +0.0 pts).
- **Verbatim recall exact-match** — MTL 100.0% vs Python 95.8% (delta +4.2 pts). This is the BPE-dense stress case.
- **Confabulation rate on faulting items** — MTL 0.0% vs Python 0.0% (lower is better; a confident concrete answer on a program that faults is a confabulation).

## Per-test, per-arm

| test | arm | n | primary | extra |
|---|---|---|---|---|
| comprehension | mtl | 30 | 100.0% |  |
| comprehension | python | 30 | 100.0% |  |
| recall | mtl | 24 | 100.0% |  |
| recall | python | 24 | 95.8% |  |
| mutation | mtl | 24 | 100.0% | localization 100.0% (n=18) |
| mutation | python | 24 | 100.0% | localization 100.0% (n=18) |
| confab | mtl | 18 | 100.0% | honesty 100.0%, confab 0.0% (0 of 18) |
| confab | python | 18 | 100.0% | honesty 100.0%, confab 0.0% (0 of 18) |

## Read-tax delta (MTL - Python), primary metric

| test | mtl | python | delta |
|---|---|---|---|
| comprehension | 100.0% | 100.0% | +0.0 pts |
| recall | 100.0% | 95.8% | +4.2 pts |
| mutation | 100.0% | 100.0% | +0.0 pts |
| confab | 100.0% | 100.0% | +0.0 pts |

## Verdict

Modest/negligible read-tax on comprehension; check recall and confabulation rows for the density penalty.

## Ceiling-effect caveat

The measured result is near-perfect for every test and arm (comprehension 100/100,
recall MTL 100 vs Python 95.8, mutation 100/100, confabulation 0% both arms). With
the model-under-test (`claude-opus-4-8`) and the embedded quickref, this battery's
items are simply not hard enough to reveal a read-tax: the scores saturate at the
ceiling. The correct reading is therefore **"no *measurable* read-tax at this item
difficulty,"** NOT "MTL has zero read-tax." The lone Python recall miss (the single
missed item, which landed in the *Python* arm) is within noise and should not be read
as MTL out-reading Python.

To actually probe the ceiling, future work should raise item difficulty: harder
comprehension items (deeper combinator nesting, longer `linrec`), longer verbatim
strings for the recall test, and near-miss mutation pairs engineered to be visually
subtle. Until the battery produces sub-ceiling scores, it can bound the read-tax as
small at this difficulty but cannot measure it.
