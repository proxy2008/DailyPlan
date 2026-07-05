// DailyPlan 多日打卡表 Typst 模板
// data.json 是 PrintData 数组，每天渲染一页，最后一天不分页。
// 多日模式：不处理待定（pending 全 false）、不重排、note 来自 task.description。

#let days = json("data.json")

#set page(
  paper: "a4",
  margin: (top: 1.2cm, bottom: 1.0cm, left: 1.2cm, right: 1.2cm),
)
#set text(
  font: ("PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", "Source Han Sans SC", "Heiti SC"),
  size: 10pt,
  lang: "zh",
)
#set par(leading: 0.65em, spacing: 0.6em)

// 复选框：画出来，不依赖 U+2610 字形。
#let checkbox() = box(baseline: 38%)[
  #rect(width: 1.05em, height: 1.05em, stroke: 0.8pt, radius: 1.5pt, fill: white)
]

// 顶部统计徽章
#let badge(content) = box(
  fill: rgb("f0f2f5"),
  inset: (x: 0.5em, y: 0.2em),
  radius: 3pt,
  stroke: 0.5pt + gray,
)[#content]

// 渲染单天内容（标题 + 冲突 + 表格 + 复盘），不含 set page。
#let render-day(data) = {
  // ===== 标题区 =====
  grid(
    columns: (1fr, auto),
    align: (left, right),
    text(size: 17pt, weight: "bold")[#data.title],
    text(size: 10pt)[
      #badge[#data.date]
      #h(0.3em)
      #badge[#data.weekday_cn]
      #h(0.3em)
      #badge[共 #data.items.len() 项]
    ],
  )

  v(0.2em)
  line(length: 100%, stroke: 1.2pt + black)
  v(0.3em)

  // ===== 冲突告警（如有）=====
  if data.conflicts.len() > 0 [
    block(
      width: 100%,
      fill: rgb("fff4e5"),
      stroke: (left: 3pt + rgb("e8a33d")),
      inset: 6pt,
      radius: 2pt,
    )[
      #set text(size: 9pt, fill: rgb("b0651a"))
      #text(weight: "bold")[⚠ 时段冲突]
      #v(0.2em, weak: true)
      #set par(leading: 0.5em)
      #for c in data.conflicts [
        #c \
      ]
    ]
    v(0.3em)
  ]

  // ===== 打卡表 =====
  if data.items.len() > 0 [
    table(
      columns: (2.3cm, 1fr, 1.8cm, 1.3cm, 1fr),
      column-gutter: 0pt,
      align: (center, left, center, center, left),
      stroke: 0.6pt + black,
      inset: 6.5pt,
      table.header(
        table.cell(fill: rgb("f0f2f5"))[*时间*],
        table.cell(fill: rgb("f0f2f5"))[*任务*],
        table.cell(fill: rgb("f0f2f5"))[*时长*],
        table.cell(fill: rgb("f0f2f5"))[*完成*],
        table.cell(fill: rgb("f0f2f5"))[*备注*],
      ),
      ..data.items.map(it => {
        // 多日模式：time=null 留空，不渲染 pending 灰底
        let time-cell = if it.time == none { none } else { it.time }
        (
          table.cell(time-cell),
          table.cell(it.task_name),
          table.cell(if it.duration_min > 0 { [ #it.duration_min 分 ] } else { none }),
          table.cell(checkbox()),
          table.cell(it.note),
        )
      }).flatten(),
    )
  ] else [
    align(center)[
      #v(1em)
      #text(size: 11pt, fill: gray)[今日暂无计划任务]
      #v(1em)
    ]
  ]

  if data.with_review [
    v(0.8em)
    let writing-box(title) = block(width: 100%)[
      #text(weight: "bold", size: 10.5pt)[#title]
      #v(0.2em)
      block(
        width: 100%,
        height: 2.6cm,
        clip: true,
        stroke: 0.6pt + black,
        radius: 2pt,
        inset: (x: 6pt, y: 4pt),
      )[
        #v(0.55cm) #line(length: 100%, stroke: 0.4pt + luma(210))
      ]
    ]
    writing-box("今日复盘")
    v(0.5em)
    writing-box("明日改进")
  ]
}

// 循环渲染每一天，最后一天后不 pagebreak
for (i, day) in days.enumerate() {
  render-day(day)
  if i < days.len() - 1 {
    pagebreak()
  }
}
