#!/bin/bash
# Hard rule (user, 2026-11-07): no source file over 700 lines.
# Covers all .rs files in the workspace; exits non-zero on any violation.
set -u
cd "$(dirname "$0")"
limit=700
fail=0
while IFS= read -r f; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$limit" ]; then echo "OVER $n  $f"; fail=1;
  fi
done < <(find crates -name '*.rs' -not -path '*/target/*' | sort)
echo "checked: $(find crates -name '*.rs' -not -path '*/target/*' | wc -l) .rs files, limit $limit"
exit $fail
