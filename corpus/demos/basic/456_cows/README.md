# 456 cows

Narayana 奶牛数（`N(9)=19`；449 的同构版）。

- 对标：449 padovan → 同递推异初值语义（`N(6)=6`）
- 绕行：`n=0/1/2` 三提前返回（449 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/456_cows/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/456_cows/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/456_cows/main.sla -o /tmp/basic_456 && /tmp/basic_456
# 期望输出：19,6,1
```
