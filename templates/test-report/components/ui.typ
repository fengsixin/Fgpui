// 通用排版组件
#let meta-table(rows) = table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: 0.5pt + luma(200),
  ..rows.map(r => (r.at(0), r.at(1))).flatten(),
)

#let revisions-table(revisions) = {
  if revisions.len() > 0 [
    == 修订记录
    #table(
      columns: (auto, auto, auto, 1fr),
      inset: 6pt,
      stroke: 0.5pt + luma(180),
      table.header([*版本*], [*日期*], [*修订人*], [*修订说明*]),
      ..revisions.map(r => (
        [#r.at("version", default: "")],
        [#r.at("date", default: "")],
        [#r.at("author", default: "")],
        [#r.at("description", default: "")],
      )).flatten(),
    )
  ]
}
