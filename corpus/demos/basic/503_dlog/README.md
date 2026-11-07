# 503 dlog

离散对数小例（`3^x≡6 → x=3`；485 的逆问题版）。

- 对标：485 primroot → 暴力枚举指数（无解返 0；`x=0` 与无解同值属小例约定）
- 绕行：`b % p` 归一目标（`powmod(·,0,·)=1` 天然处理 `x=0`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/503_dlog/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/503_dlog/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/503_dlog/main.sla -o /tmp/basic_503 && /tmp/basic_503
# 期望输出：3,2,0
```
