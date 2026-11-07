# 881 motz_riob11

Motzkin-Riordan 续桥三 `R(11)=1585`，`R(12)=4213`，`M(11)=1585+4213=5798`（桥恒等续高位）。

- 对标：751_motz_riob9 × 876_motz11 → 两族桥接续高位（883 做续桥对峙）
- 绕行：桥恒等 `M(n)=R(n)+R(n+1)` 双值连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/881_motz_riob11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/881_motz_riob11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/881_motz_riob11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/881_motz_riob11/main.sla -o /tmp/basic_881 && /tmp/basic_881
# 期望输出：1585,4213,5798
```
