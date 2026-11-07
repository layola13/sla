# 751 motz_riob9

Motzkin–Riordan 续桥 `R(9)=232`，`R(10)=603`，`M(9)=232+603=835`（桥恒等续高位）。

- 对标：643_rio_bridge → 低位桥互补（746 高位源合流；753 做续桥对峙）
- 绕行：桥恒等 `M(n)=R(n)+R(n+1)` 高位再验（条件内加法断言，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/751_motz_riob9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/751_motz_riob9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/751_motz_riob9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/751_motz_riob9/main.sla -o /tmp/basic_751 && /tmp/basic_751
# 期望输出：232,603,835
```
