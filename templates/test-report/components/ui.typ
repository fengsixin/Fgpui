// 通用排版组件
#let meta-table(rows) = table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: 0.5pt + luma(200),
  ..rows.map(r => (r.at(0), r.at(1))).flatten(),
)
