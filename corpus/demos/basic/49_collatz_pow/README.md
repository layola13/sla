# 49 Collatz Pow

对标 `tsgosa/demos/155_collatz` + `135_pow_loop`。

- `collatz_steps(27) = 111`：`while + if/else` 长循环（覆盖奇偶分支）。
  注意计数口径：每次变换计 1（文献常记 112 含起点，实测本函数输出 111，已用 build-exe 核对）。
- `pow2(10) = 1024`：`for` 累乘。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/49_collatz_pow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/49_collatz_pow/main.sla
```
