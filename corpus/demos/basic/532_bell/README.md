# 532 bell

Bell 数 `B(n) = Σ_k S(n,k)`（`B(3..5)=5,15,52`；Stirling 行求和）。

- 对标：531_stirling2 → 行版互补（531 取单格，此处行求和）；453_bell → 集合版互补
- 绕行：内层上界 `kk <= nn` 收窄（上三角恒零区不写，避无效下标）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/532_bell/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/532_bell/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/532_bell/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/532_bell/main.sla -o /tmp/basic_532 && /tmp/basic_532
# 期望输出：15,52
```
