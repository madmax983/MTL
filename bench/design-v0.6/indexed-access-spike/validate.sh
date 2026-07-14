#!/usr/bin/env bash
# Reproduce the spike validation. Run from this directory.
set -e
rustc -O spike.rs -o spike
echo "== binary_search =="
for v in "1 0" "3 1" "5 2" "7 3" "9 4" "4 -1" "0 -1" "10 -1"; do
  set -- $v; t=$1; exp=$2
  got=$(./spike @binary_search.mtl '[1 3 5 7 9]' "$t")
  echo "  t=$t expect=$exp  ->  $got"
done
echo "== two_sum =="
./spike @two_sum.mtl '[2 7 11 15]' 9
./spike @two_sum.mtl '[3 2 4]' 6
./spike @two_sum.mtl '[1 3 5 7 9]' 12
./spike @two_sum.mtl '[3 3]' 6
