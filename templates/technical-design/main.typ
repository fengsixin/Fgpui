// 技术方案模板入口。数据契约：schema.json；示例：examples/sample.json
// 结构：封面 → 目录（页眉页脚/页码）→ 修订记录 → 摘要 → 正文（Typst 源码自由录入）
#let data = json("data.json")
#import "theme.typ": apply-theme
#import "components/ui.typ": meta-table, revisions-table

#show: apply-theme

// 封面（无页码）
#set page(numbering: none, header: none, footer: none)
#align(center + horizon)[
  #v(-2.5cm)
  #text(size: 24pt, weight: "bold")[#data.title]
  #if data.at("subtitle", default: "") != "" [
    #v(0.5em)
    #text(size: 14pt, fill: luma(100))[#data.subtitle]
  ]
  #v(1.6em)
  #meta-table((
    ([*作者 / 部门*], [#data.at("author", default: "")]),
    ([*文档版本*], [#data.at("version", default: "")]),
    ([*编写日期*], [#data.at("date", default: "")]),
    ([*密级*], [#data.at("classification", default: "")]),
  ))
]

// 正文区：页眉（文档标题 + 密级）与页脚（第 X 页 / 共 Y 页）
#set page(
  numbering: "1",
  header: align(right)[#text(size: 9pt, fill: luma(120))[#data.title ｜ 密级：#data.at("classification", default: "")]],
  footer: context align(center)[#text(size: 9pt)[第 #counter(page).display() 页 / 共 #counter(page).final().first() 页]],
)

#pagebreak()
#outline(title: [目录], depth: 3)
#pagebreak()

#revisions-table(data.at("revisions", default: ()))

#if data.at("summary", default: "") != "" [
  == 摘要
  #data.summary
]

#let body-src = data.at("body", default: "")
#if body-src != "" [
  #eval(body-src, mode: "markup")
]
