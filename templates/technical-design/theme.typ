// 文档主题：中英文混排基线（正式分发的字体在阶段 4/6 由应用字体目录提供）
#let apply-theme(doc) = {
  set page(paper: "a4", margin: (x: 2.2cm, y: 2.2cm), numbering: "1 / 1", number-align: center)
  set text(
    font: ((name: "Microsoft YaHei", covers: "latin-in-cjk"), "Segoe UI"),
    size: 11pt,
    lang: "zh",
    region: "cn",
  )
  set par(justify: true, leading: 0.78em)
  set heading(numbering: "1.1.")
  doc
}
