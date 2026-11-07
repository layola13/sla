# 479 mediant

中项分数（`mediant(1/3,1/2)=2/5`；478 的构造版）。

- 对标：478 farey_len → 分子分母交叉相乘定位（`5<6` 且 `4<5` 居间）
- 绕行：百分位打包（分子分母皆小量；`>=` 即判出界）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/479_mediant/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/479_mediant/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/479_mediant/main.sla -o /tmp/basic_479 && /tmp/basic_479
# 期望输出：205,1
```
