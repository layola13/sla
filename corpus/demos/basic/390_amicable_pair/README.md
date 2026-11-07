# 390 amicable pair

亲和数对验证（`s(220)=284` 互映；335 的函数版）。

- 对标：335 amicable → 真因子和函数 + 双向互映判定（自对判 0）
- 绕行：无（`i < n` 排除自身；早返回三段式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/390_amicable_pair/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/390_amicable_pair/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/390_amicable_pair/main.sla -o /tmp/basic_390 && /tmp/basic_390
# 期望输出：284,220,1
```
