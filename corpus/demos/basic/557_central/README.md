# 557 central

中心二项式 `B(n)=C(2n,n)`：乘法式逐项整除（350 范式）。`B(4..6)=70,252,924`。

- 对标：350_binom → 单值版延展（此处 `2n` 取顶；351/558 供商底）
- 绕行：逐项精确整除（350 同例）；`B(0)=1` 空积端点

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/557_central/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/557_central/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/557_central/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/557_central/main.sla -o /tmp/basic_557 && /tmp/basic_557
# 期望输出：70,252,924
```
