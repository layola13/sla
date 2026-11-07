# 516 golomb

Golomb 自描述数列（`G(9)=5`；448 的自指版）。

- 对标：448 tribonacci → `1+G(n-G(G(n-1)))` 双重递归（`G(4)=3`）
- 绕行：`n==1` 单基（内层先收敛；深度 10 安全）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/516_golomb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/516_golomb/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/516_golomb/main.sla -o /tmp/basic_516 && /tmp/basic_516
# 期望输出：5,5,3
```
