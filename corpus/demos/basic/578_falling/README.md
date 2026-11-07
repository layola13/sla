# 578 falling

下降阶乘 `fall(n,x)=x·(x-1)…(x-n+1)`（`fall(4,4)=24`，`fall(5,5)=120`，`fall(2,5)=20`）。

- 对标：577_rising → 上升版互补（579 做升降对峙；572 连乘同形）
- 绕行：签名语义为（项数，起点），`P(5,2)` 写作 `falling(2,5)`（初版误作 `falling(5,2)` 得 0，panic 57803 定位修正）；`falling(3,1)=0` 含零因子端点

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/578_falling/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/578_falling/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/578_falling/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/578_falling/main.sla -o /tmp/basic_578 && /tmp/basic_578
# 期望输出：24,120,20
```
