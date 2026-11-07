# 553 sym

对称性对峙 `C(n,k) == C(n,n-k)`（`C(10,3)=120` 对 `C(10,7)=120`；`C(9,2)=36` 对 `C(9,7)=36`）。

- 对标：546_pascal → 单实现见源（表内对称天然成立，双下标互查锁定）
- 绕行：宽端 `k=7` 建表更宽但同式绿（`w=k+1` 自适应，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/553_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/553_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/553_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/553_sym/main.sla -o /tmp/basic_553 && /tmp/basic_553
# 期望输出：120,120,36,36
```
