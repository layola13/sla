# 580 rowval

第一类 Stirling 行值 `Σ_k c(n,k) = n!`（`row3=6`，`row4=24`，`row5=120`）。

- 对标：576_stirling1 → 单格版互补（532/570 行和体例；581 做阶乘对峙）
- 绕行：`k` 自 1 起（`c(n,0)=0` 恒零区不读，532 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/580_rowval/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/580_rowval/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/580_rowval/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/580_rowval/main.sla -o /tmp/basic_580 && /tmp/basic_580
# 期望输出：6,24,120
```
