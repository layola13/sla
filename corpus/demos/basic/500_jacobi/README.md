# 500 jacobi

Jacobi 符号（`(2|21)=-1`；499 的合数版）。

- 对标：499 legendre → 素因子分解连乘（`(2|15)=1`，`(3|15)=0`）
- 绕行：`±1/0` 三值显式（`0-1` 表负；嵌套 `while` 内层不加 `;`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/500_jacobi/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/500_jacobi/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/500_jacobi/main.sla -o /tmp/basic_500 && /tmp/basic_500
# 期望输出：1,-1,0
```
