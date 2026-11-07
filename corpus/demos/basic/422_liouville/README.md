# 422 liouville

Liouville 累和（`L(10)=0`；421 的全计数版）。

- 对标：421 mertens → Ω 计重数定符号（`λ(1)=1`，空积为偶）
- 绕行：嵌套 `while` 内层不加 `;`（386 同例）；`0 - 1` 表负数

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/422_liouville/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/422_liouville/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/422_liouville/main.sla -o /tmp/basic_422 && /tmp/basic_422
# 期望输出：0,-2,1
```
