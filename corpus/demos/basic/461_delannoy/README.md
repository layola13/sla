# 461 delannoy

Delannoy 数（`D(3,3)=63`；360 的格路版）。

- 对标：360 cube_sum → 三向递推（`D(2,2)=13`，`D(1,1)=3`）
- 绕行：双基 `m==0/n==0` 归 1；小规模递归安全

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/461_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/461_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/461_delannoy/main.sla -o /tmp/basic_461 && /tmp/basic_461
# 期望输出：63,13,3
```
