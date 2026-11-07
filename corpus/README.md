# corpus — sa_plugin_sla 语料镜像（demos + 全部单元测试）

本目录是 Zig 版仓 `layola13/sa_plugin_sla` 的 `tests/` 与 `demos/`
两棵树的**逐字节镜像**，供 Rust 独立二进制做全量对拍普查
（同口径分母）。镜像基线见 `MANIFEST.tsv` 首行注释：

- 源仓：`layola13/sa_plugin_sla`
- 源头：`753fd682b0b65bd7ecab90162ee91c2f0b1846c9`（master，2026-11-07 lang-gap 落地后）
- 文件数：4415（tests 777 + demos 3638）

## 纪律

- **禁止手改语料**：corpus 下任何文件不得直接编辑。Rust 侧与 Zig 侧
  的行为差异只经普查登记（见 `CENSUS.md`），修复走源码线，不动语料。
- **刷新规程**（源仓推进后重镜像）：
  1. 在源仓的具名新头上取 `tests/`、`demos/` 的 git 跟踪文件集
     （`git ls-files tests demos`），整树替换 `corpus/tests`、
     `corpus/demos`，不做增量挑拣；
  2. 重新生成 `MANIFEST.tsv`（每文件一行：相对路径、字节数、
     sha256；首行注释更新源头 sha 与文件数）；
  3. 对账：按 MANIFEST 重算的聚合哈希（排序后逐文件
     `sha256  path` 行再 sha256）须与源仓同法计算值逐字相等，
     文件数须与 `git ls-files` 计数相等；对不上不提交；
  4. 以独立提交落地，提交信息注明源头 sha 与文件数。
- 本目录为镜像件，**不受单文件 700 行上限约束**（该上限只管源码）。

## 普查

首轮全量普查结果与口径见同目录 `CENSUS.md`（分母、双后端判读、
与 Zig 同头参照的三分类名集：双侧同绿 / Zig 绿 Rust 红（真缺口）/
双侧同红（存量））。
