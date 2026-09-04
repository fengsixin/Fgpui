// Fgpui 阶段 0 冒烟测试模板（固定内容，不要修改）
// 由 Rust 后端写入临时目录后调用内置 Typst sidecar 编译。
#set page(paper: "a4", margin: 2cm)
#set text(size: 11pt)

= Fgpui Typst 自检报告

本文档由 Fgpui 内置 Typst 编译器（sidecar）生成，用于验证「调用内置 Typst → 编译测试模板 → 生成 PDF」完整链路。

== 检查项

#table(
  columns: (auto, auto),
  inset: 8pt,
  align: (left, left),
  [*检查项*], [*结果*],
  [Typst sidecar 调用], [通过],
  [模板编译], [通过],
  [PDF 输出], [通过],
)

== 中英文混排

标准文档生成工具（Standard Document Generator）需要稳定的中英文混排能力：
The quick brown fox jumps over the lazy dog. 敏捷的棕色狐狸跳过了懒惰的狗。

== 第二页

#pagebreak()

多页输出验证页：阶段 0 编译链路自检通过。
