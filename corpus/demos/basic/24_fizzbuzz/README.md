# 24 Fizzbuzz

对标 `tsgosa/demos/191_fizz20`。

`||` 短路 + `%` 整除判断。注意 `for i in 1..(limit + 1)` 的右端点需括号
（`1..limit + 1` 会被解析为 `(1..limit) + 1`，此处显式加括号）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/24_fizzbuzz/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/24_fizzbuzz/main.sla
```
