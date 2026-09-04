// 通用排版组件
#let meta-table(rows) = table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: 0.5pt + luma(200),
  ..rows.map(r => (r.at(0), r.at(1))).flatten(),
)

#let chapter-table(tbl) = {
  let cols = tbl.at("columns", default: ())
  let rows = tbl.at("rows", default: ())
  if tbl.at("caption", default: "") != "" [
    #align(center)[#text(size: 9pt)[#strong[#tbl.caption]]]
  ]
  table(
    columns: cols.len(),
    inset: 6pt,
    stroke: 0.5pt + luma(180),
    table.header(..cols.map(c => strong[#c])),
    ..rows.map(r => r.at("cells", default: ()).map(c => [#c])).flatten(),
  )
}

#let figure-block(img) = figure(
  image(img.at("path", default: "assets/missing.png"), width: (img.at("widthCm", default: 10) * 1cm)),
  caption: img.at("caption", default: ""),
)
