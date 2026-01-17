# 給 Claude 的實作提示詞：Rust MCP（檔案路徑）+ Yoga 版面計算 → 16:9 橫式投影片 Layout Plan

你是資深 Rust 工程師。請依本文件，實作一個 **MCP Server**（stdio transport），提供工具讓上層 AI 以「檔案路徑」輸入一份 Markdown，MCP 解析內容後用 **Yoga (Flexbox)** 計算一張 **16:9 橫式投影片**中所有元素（文字方塊、圖片/示意圖佔位區）的 **座標與大小**，輸出 `layout.json`（以及 `report.json`）。

上層 AI 會在拿到 `layout.json` 後，用 python/pptx 或 pywin32 依座標把投影片真正畫出來。因此你的 MCP 只要負責：**解析 Markdown → 建立 Layout Tree → Measure（文字/表格高度）→ Yoga 計算 → 輸出 Layout Plan**。

---

## 0. 成功標準（必達）
1. Tool **只接受檔案路徑**：`markdown_path`（不接受 inline markdown）。
2. Markdown 同時承載：
   - **文字內容**
   - **示意圖/圖片佔位的長寬比例**（例如 16:9、4:3、1:1、3:2 或任意 w:h）
3. MCP 輸出：
   - `layout.json`：每個元素 `id` 的 `x,y,w,h`（以 pt 為單位）
   - `report.json`：overflow、縮排/降級、警告（例如內容過長）
4. 不允許 Markdown 指定任何絕對尺寸（font size、px、pt、margin…），只允許 **語意** + **圖形比例**。
5. 版面要 deterministic：同一份 md + 同一份 theme/config → 結果一致。

---

## 1. Tool Contract（MCP）
### 1.1 Tool 名稱
- `layout.compute_slide_layout`

### 1.2 Input（JSON）
```json
{
  "markdown_path": "workspace/inputs/slide.md",
  "theme_path": "workspace/themes/default.json",
  "output_dir": "workspace/out",
  "slide": {
    "aspect": "16:9",
    "orientation": "landscape",
    "unit": "pt"
  },
  "options": {
    "template": "auto",
    "density": "compact",
    "allow_two_column": true,
    "debug_dump": false
  }
}
```

### 1.3 Output（JSON）
```json
{
  "layout_json_path": "workspace/out/layout.json",
  "report_json_path": "workspace/out/report.json"
}
```

---

## 2. 安全與路徑規範（必做）
- 只允許讀寫 `workspace/` 之下的相對路徑
- 禁止 `..`、禁止絕對路徑
- `markdown_path` 不存在 → 回 MCP error（可讀 message）
- `output_dir` 不存在 → 自動建立

---

## 3. Slide 座標系（pt）
- 16:9 橫式：**960 pt × 540 pt**
- (0,0) 在左上，x 向右、y 向下

---

## 4. Theme 檔（theme_path）
Theme 決定所有「不可由 Markdown 指定」的東西：
- typography roles：`title/subtitle/h2/body/caption/mono`
- spacing scale：`xs/sm/md/lg/xl/2xl`
- layout policy：模板切換、最小字級、最小圖片框、兩欄比例策略等

### 4.1 theme.json 範例（你可自行擴充）
```json
{
  "typography": {
    "title":   { "family": "Inter", "size_pt": 34, "line_height": 1.10, "weight": 700 },
    "subtitle":{ "family": "Inter", "size_pt": 18, "line_height": 1.20, "weight": 500 },
    "h2":      { "family": "Inter", "size_pt": 20, "line_height": 1.20, "weight": 700 },
    "body":    { "family": "Inter", "size_pt": 14, "line_height": 1.35, "weight": 400 },
    "caption": { "family": "Inter", "size_pt": 12, "line_height": 1.30, "weight": 400 },
    "mono":    { "family": "JetBrains Mono", "size_pt": 12, "line_height": 1.30, "weight": 400 }
  },
  "spacing_pt": { "xs":4, "sm":8, "md":12, "lg":16, "xl":24, "2xl":32 },
  "policy": {
    "page_padding": "xl",
    "min_font_pt": 10,
    "min_image_box_pt": { "w": 180, "h": 120 },
    "two_col_when": { "has_image_or_diagram": true, "has_bullets_or_table": true },
    "two_col_split": [0.58, 0.42]
  }
}
```

---

## 5. Markdown 輸入格式規約（關鍵）
### 5.1 支援語法（MVP）
- `#`：投影片 Title（只能 1 個）
- `>`：Subtitle（只有 Title 後第一個 blockquote 視為 subtitle；其餘視為 note/callout）
- `##`：Section
- `-`：Bullets（最多 2 層）
- Markdown Table（GFM）
- `**bold**`、`` `code` ``：inline marks（只影響語意樣式，不影響布局參數）
- **圖片/示意圖佔位**：用一種固定語法在 Markdown 內表達「比例」與「描述」

### 5.2 圖片/示意圖佔位語法（必做）
使用 HTML-like 單行 tag（但**不得**帶 style/尺寸），只允許以下屬性：
- `id`：元素 id（必填，唯一）
- `ratio`：`w:h`（必填，例如 `16:9`, `4:3`, `1:1`, `3:2`, `21:9`）
- `kind`：`image|diagram|chart`（選填，預設 diagram）
- `alt`：自然語言描述（必填，用來生成圖片或畫示意圖）

格式：
```markdown
<fig id="flow" ratio="16:9" kind="diagram" alt="資料流：App -> SDK -> Service -> libgui -> SharedMemory -> App" />
```

> 注意：你只要把 `<fig .../>` 解析成一個 Leaf Node（圖片框），並以 `ratio` 決定它在給定 width 下的 height（h = w * ratio_h/ratio_w）。

---

## 6. 解析與語意化（Markdown → DocAST）
你需要把 Markdown 解析成固定 AST 結構（自定義資料型別）：
- `Slide { title, subtitle?, blocks[] }`
- `Block` 類型：
  - `Section { heading, children[] }`
  - `Bullets { items[] }`
  - `Table { header[], rows[][] }`
  - `Callout { text }`
  - `Figure { id, ratio, kind, alt }`

規則：
1. 第一個 `#` → title
2. title 後第一個 `>` → subtitle
3. `##` 開一個 section，直到下一個 `##` 或 EOF
4. section 內可包含 bullets/table/callout/figure（順序保留）
5. `<fig .../>` 可以出現在 section 內任何位置

---

## 7. Layout 策略（DocAST → Yoga Tree）
### 7.1 高階模板（先只做 2 種）
- `single_col`：內容從上到下排
- `two_col`：body 區域左右兩欄（左文字，右 figure），適用「同時有文字+圖」

### 7.2 auto 模板選擇（options.template = auto）
- 若存在任何 `<fig .../>` 且同時存在 bullets/table → 用 `two_col`
- 否則 `single_col`

### 7.3 結構建議（示意）
- root: column（padding=theme.policy.page_padding，gap=lg）
  - header: column（title、subtitle）
  - body: 
    - two_col：row（split=policy.two_col_split）
      - left: column（sections 的文字與表格）
      - right: column（figures）
    - single_col：column（sections 依序）
  - footer: callout（可選，或 section 內 note）

> 你需要保留元素 id，讓 output layout.json 能對應回原始內容位置。

---

## 8. Yoga 實作要點（核心）
### 8.1 使用 Yoga 進行排版
- Container nodes：設定 flexDirection、flexGrow、padding、margin、gap（gap 可用 children margin 模擬）
- Leaf nodes：Text、Table、Figure 都必須能被 measure（至少要回傳 height）

### 8.2 文字量測（必做，簡化可接受）
Yoga 不知道文字高度，你要實作：
- `measure_text(role, text, max_width_pt, theme) -> (w,h)`
最低可行方案（先用近似）：
- 估算每行可容納字元數：`chars_per_line = floor(max_width_pt / avg_char_width_pt)`
- 由 text 長度估算行數（處理換行）
- `height = lines * (font_size_pt * line_height) + paragraph_spacing`
平均字寬可用 `font_size_pt * 0.55`（中英混排可用保守值）

> MVP 不要求字形精準，只要不大量 overflow；之後可換 fontdue/rusttype 做精準測量。

### 8.3 Table 量測（必做，簡化可接受）
- 假設欄寬等分（或依 header 長度加權）
- 每格文字依欄寬做換行估算
- row height = max(cell heights) + row padding
- total height = header + sum(rows)

### 8.4 Figure 量測（必做）
- 給定 `max_width_pt`：
  - `w = max_width_pt`（或受 min_image_box 約束）
  - `h = w * ratio_h/ratio_w`
- 若 `h` 過高導致 overflow：在 review 階段回報，或在策略階段改模板/密度

---

## 9. Overflow / Review（必做）
產出 `report.json`：
- `overflow_elements[]`：元素超出 slide bounds
- `clipped_text[]`：文字高度估算 > 分配 box 高度（代表會截斷）
- `fallbacks[]`：採用的降級策略（例如 two_col → single_col、density=compact）
- `warnings[]`：例如 figure 太多、表格太寬、bullets 太長

最低限度降級策略（按順序嘗試）：
1. two_col → single_col
2. density: comfortable → compact（減 gap/padding）
3. 仍 overflow：回報 warning（要求上層 AI 精簡文字）

---

## 10. 輸出格式：layout.json（給後續 AI 產 PPTX）
layout.json 必須包含：
- slide size
- 全元素列表（包含 text/table/figure/callout）
- 每個元素：
  - `id`（唯一，可用路徑式：`section:核心機制:bullets0`、`fig:flow`）
  - `kind`：`text|table|bullets|figure|callout`
  - `role`：`title|subtitle|h2|body|caption|mono`
  - `box`：`x,y,w,h`（pt）
  - `source_ref`：指向 markdown AST 的位置（debug 用）

範例：
```json
{
  "slide": { "w_pt": 960, "h_pt": 540 },
  "elements": [
    { "id": "title", "kind": "text", "role": "title", "box": { "x": 24, "y": 24, "w": 912, "h": 44 } },
    { "id": "fig:flow", "kind": "figure", "role": "body", "ratio": "16:9", "alt": "資料流...", "box": { "x": 560, "y": 140, "w": 376, "h": 211 } }
  ]
}
```

---

## 11. 專案結構（建議）
```
mcp-yoga-layout/
  Cargo.toml
  src/
    main.rs            # MCP server + tool registry
    tool.rs            # compute_slide_layout handler
    paths.rs           # workspace path sanitize
    theme.rs           # theme json structs
    md.rs              # markdown parse -> DocAST
    ast.rs             # DocAST types
    layout/
      templates.rs     # single_col / two_col builders
      yoga.rs          # yoga wrapper (safe RAII)
      measure.rs       # text/table/figure measurement
      compute.rs       # run yoga + extract boxes
      review.rs        # overflow & fallback
    output.rs          # write layout.json/report.json
  workspace/
    inputs/
    themes/
    out/
```

---

## 12. 測試用 Markdown（你必須用它驗收）
把以下內容存成 `workspace/inputs/slide.md`，跑 tool 應得到合理 boxes（不 overflow 或有 fallback 記錄）：

```markdown
# Anti-Lag POC
> 目標：降低輸入延遲，並保留穩定幀率

## KPI
| 指標 | 數值 | 變化 |
|---|---:|---:|
| Input Lag | **16ms** | -60% |
| Avg Power | 3.1W | -18% |

## 核心機制
- CPU 不再超前排隊 3-4 幀
- 只保持 **1 幀** queue
- `Fence` 同步點插入引擎與 GPU 之間

## 資料流示意
<fig id="flow" ratio="16:9" kind="diagram" alt="Pipeline：Game App -> SDK -> Service -> libgui -> SharedMemory -> Game App，右側註記：queue=1" />

> Note：若內容過長，優先改 single-column，其次 compact，再回報需精簡。
```

---

## 13. 交付內容（Claude 你要產出什麼）
1. 可編譯的 Rust MCP server（stdio）
2. tool：`layout.compute_slide_layout`
3. 完整的 markdown parser（支援上面規約）
4. Yoga wrapper + measure functions（text/table/figure）
5. layout.json + report.json 輸出
6. 最少 3 個 test cases（短/長文字/多 figures），並在 report 中呈現 fallback

---

## 14. 實作注意（你常見會踩坑）
- Yoga 的 measure callback 生命週期：確保 Rust closure 與 user_data 安全（避免 use-after-free）
- gap：若你用的 Yoga 版本不支援 gap，改用 child margin 模擬
- 兩欄：右欄 figures 多個時，右欄也要是 column（自動往下排）
- 表格寬度：不追求精準，只要合理換行估算，避免總寬爆掉
- report 要可讀：上層 AI 需要看 report 才能自動縮內容

---

請直接開始設計與寫碼，先讓 MVP 跑起來，再逐步把文字量測從「估算」替換成 fontdue/rusttype 精準量測。
