# FB-038 — check 命令内联复刻前端管线，与 plugin_compile 主干构成双路漂移面

- **反馈环批次**：第四批（三期 HND 评估线发现、协调线收编）
- **源登记**：`docs/phase3/handler-eval.md` §6 HND-C4
- **分类**：Zig 侧候选（观察级，结构登记）—— **未做双路差分
  实测**，本单只登记结构事实，不声称已证缺陷
- **Rust 侧处置**：描述符 handle_command 的 check 命令（三期 A0
  切片外）未接线；仅登记。

## 结构事实（Zig 源码位置，本地参照树行号）

- check 命令在 `src/plugin_commands.zig` 内联复刻前端管线，
  而非共用 `src/plugin_compile.zig:591-807` 的主干：参数写死
  形态包括 `parse_test_bodies=false`、
  `include_all_imported_decls=true`、无 freshness 段。
- build 与 check 两路的参数与阶段由此长期独立演化，构成漂移
  面：同一程序在两路出现不同判定将无任何现行门能发现（本线
  对拍 e2e 走的是 compile 主干，check 路不在对拍面内）。

## 建议后续（仅建议）

主线若保留双路形态，至少以一组同程序 build/check 判定对照
语料钉住两路一致性；若收编为共用主干，本单自动销账。Rust 侧
四期接 check 命令前应先索取该对照结论，避免把漂移当口径
镜像。
