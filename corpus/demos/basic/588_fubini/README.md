# 588 fubini

有序 Bell 数（Fubini）：`F(n)=Σ_k k!·S(n,k)`（`F(3)=13`，`F(4)=75`，`F(5)=541`）。

- 对标：532_bell → 无序版互补（此处 `k!` 加权行和；复用 531 二类表，591 做对峙）
- 绕行：`F(5)=1+30+150+240+120` 分项手算复核（记 README）；`fact(k)·S` 混算 528 同例

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/588_fubini/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/588_fubini/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/588_fubini/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/588_fubini/main.sla -o /tmp/basic_588 && /tmp/basic_588
# 期望输出：13,75,541
```
