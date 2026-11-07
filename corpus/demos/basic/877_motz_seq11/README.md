# 877 motz_seq11

Motzkin 序列前 12 项和 `9360`（`M(11)=5798`；前缀和循环求和）。

- 对标：876_motz11 → 单值版互补（同式序列复算；前缀和三档连验）
- 绕行：`3562+5798=9360` 累加手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/877_motz_seq11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/877_motz_seq11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/877_motz_seq11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/877_motz_seq11/main.sla -o /tmp/basic_877 && /tmp/basic_877
# 期望输出：9360,3562
```
