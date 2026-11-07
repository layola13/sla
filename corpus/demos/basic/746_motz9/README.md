# 746 motz9

Motzkin 续高位 `M(9)=835`（`M(8)=323` 低位锚；卷积和 `512`）。

- 对标：634_motz8 → `n=8` 版互补（同式再上一位；749 做迭代/直和对峙）
- 绕行：八项卷积 `127+51+42+36+36+42+51+127=512` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/746_motz9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/746_motz9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/746_motz9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/746_motz9/main.sla -o /tmp/basic_746 && /tmp/basic_746
# 期望输出：323,835
```
